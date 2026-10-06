//! Just enough of the protobuf wire format for Mapbox Vector Tiles. Hand-rolled rather than
//! `prost` + `protoc`: the MVT schema is tiny and frozen, and this keeps the build free of a native
//! codegen step (and of a dependency that ships in the browser bundle).
//!
//! The reader is strict: truncated varints, lengths past the end and unknown wire types are errors,
//! never panics or silent misreads.

use crate::GeoError;

pub(crate) const WIRE_VARINT: u8 = 0;
pub(crate) const WIRE_FIXED64: u8 = 1;
pub(crate) const WIRE_LEN: u8 = 2;
pub(crate) const WIRE_FIXED32: u8 = 5;

fn truncated(what: &str) -> GeoError {
    GeoError::format(format!("MVT: truncated {what}"))
}

/// Reads protobuf primitives out of a byte slice.
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf, pos: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// A base-128 varint (at most 10 bytes).
    pub fn varint(&mut self) -> Result<u64, GeoError> {
        let mut out = 0u64;
        for i in 0..10 {
            let Some(&b) = self.buf.get(self.pos) else { return Err(truncated("varint")) };
            self.pos += 1;
            out |= ((b & 0x7f) as u64) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(out);
            }
        }
        Err(GeoError::format("MVT: varint longer than 10 bytes"))
    }

    /// The next field tag as `(field number, wire type)`, or `None` at the end of the message.
    pub fn tag(&mut self) -> Result<Option<(u32, u8)>, GeoError> {
        if self.is_empty() {
            return Ok(None);
        }
        let key = self.varint()?;
        let field = key >> 3;
        if field == 0 || field > u32::MAX as u64 {
            return Err(GeoError::format(format!("MVT: bad field number {field}")));
        }
        Ok(Some((field as u32, (key & 7) as u8)))
    }

    /// A length-delimited run (wire type 2).
    pub fn bytes(&mut self) -> Result<&'a [u8], GeoError> {
        let len = self.varint()?;
        let end = (self.pos as u64).checked_add(len).filter(|&e| e <= self.buf.len() as u64);
        let Some(end) = end else { return Err(truncated("length-delimited field")) };
        let out = &self.buf[self.pos..end as usize];
        self.pos = end as usize;
        Ok(out)
    }

    pub fn string(&mut self) -> Result<String, GeoError> {
        Ok(String::from_utf8_lossy(self.bytes()?).into_owned())
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], GeoError> {
        let s = self.buf.get(self.pos..self.pos + N).ok_or_else(|| truncated("fixed-width field"))?;
        self.pos += N;
        let mut out = [0u8; N];
        out.copy_from_slice(s);
        Ok(out)
    }

    pub fn fixed32(&mut self) -> Result<u32, GeoError> {
        Ok(u32::from_le_bytes(self.fixed::<4>()?))
    }

    pub fn fixed64(&mut self) -> Result<u64, GeoError> {
        Ok(u64::from_le_bytes(self.fixed::<8>()?))
    }

    /// Skip a field we don't read. Groups (wire types 3/4) are deprecated and never appear in MVT.
    pub fn skip(&mut self, wire: u8) -> Result<(), GeoError> {
        match wire {
            WIRE_VARINT => self.varint().map(drop),
            WIRE_FIXED64 => self.fixed::<8>().map(drop),
            WIRE_LEN => self.bytes().map(drop),
            WIRE_FIXED32 => self.fixed::<4>().map(drop),
            w => Err(GeoError::format(format!("MVT: unsupported wire type {w}"))),
        }
    }

    /// A packed repeated `uint32` field's contents.
    pub fn packed_u32(&mut self, out: &mut Vec<u32>) -> Result<(), GeoError> {
        let mut inner = Reader::new(self.bytes()?);
        while !inner.is_empty() {
            out.push(inner.varint()? as u32);
        }
        Ok(())
    }
}

/// ZigZag decoding: MVT geometry deltas and `sint` values.
#[inline]
pub(crate) fn zigzag_decode(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

/// ZigZag encoding.
#[inline]
pub(crate) fn zigzag_encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

/// Appends protobuf primitives to a byte vector.
#[derive(Default)]
pub(crate) struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer { buf: Vec::new() }
    }

    pub fn varint(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return;
            }
            self.buf.push(b | 0x80);
        }
    }

    fn tag(&mut self, field: u32, wire: u8) {
        self.varint(((field as u64) << 3) | wire as u64);
    }

    pub fn varint_field(&mut self, field: u32, v: u64) {
        self.tag(field, WIRE_VARINT);
        self.varint(v);
    }

    pub fn bytes_field(&mut self, field: u32, data: &[u8]) {
        self.tag(field, WIRE_LEN);
        self.varint(data.len() as u64);
        self.buf.extend_from_slice(data);
    }

    pub fn string_field(&mut self, field: u32, s: &str) {
        self.bytes_field(field, s.as_bytes());
    }

    pub fn float_field(&mut self, field: u32, v: f32) {
        self.tag(field, WIRE_FIXED32);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn double_field(&mut self, field: u32, v: f64) {
        self.tag(field, WIRE_FIXED64);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// A packed repeated varint field (feature tags, geometry commands).
    pub fn packed_u32(&mut self, field: u32, values: &[u32]) {
        let mut inner = Writer::new();
        for &v in values {
            inner.varint(v as u64);
        }
        self.bytes_field(field, &inner.buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_round_trips() {
        for n in [0i64, -1, 1, -2, 2, 12345, -12345, i32::MIN as i64, i32::MAX as i64, i64::MIN, i64::MAX] {
            assert_eq!(zigzag_decode(zigzag_encode(n)), n);
        }
        assert_eq!(zigzag_encode(-1), 1);
        assert_eq!(zigzag_encode(1), 2);
    }

    #[test]
    fn varint_round_trips() {
        for v in [0u64, 1, 127, 128, 300, 16384, u32::MAX as u64, u64::MAX] {
            let mut w = Writer::new();
            w.varint(v);
            assert_eq!(Reader::new(&w.buf).varint(), Ok(v));
        }
    }

    #[test]
    fn fields_round_trip_and_truncation_errors() {
        let mut w = Writer::new();
        w.varint_field(1, 42);
        w.string_field(2, "hej");
        w.double_field(3, 1.5);
        let mut r = Reader::new(&w.buf);
        assert_eq!(r.tag(), Ok(Some((1, WIRE_VARINT))));
        assert_eq!(r.varint(), Ok(42));
        assert_eq!(r.tag(), Ok(Some((2, WIRE_LEN))));
        assert_eq!(r.string().as_deref(), Ok("hej"));
        assert_eq!(r.tag(), Ok(Some((3, WIRE_FIXED64))));
        assert_eq!(r.fixed64().map(f64::from_bits), Ok(1.5));
        assert_eq!(r.tag(), Ok(None));

        assert!(Reader::new(&[0x80]).varint().is_err());
        assert!(Reader::new(&[0x05, b'a']).bytes().is_err());
        assert!(Reader::new(&[1, 2]).fixed32().is_err());
        assert!(Reader::new(&[]).skip(3).is_err());
        assert!(Reader::new(&[0xff; 11]).varint().is_err());
    }
}

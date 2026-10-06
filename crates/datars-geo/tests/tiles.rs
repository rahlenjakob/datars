//! Tile formats end to end: tile math, MVT encode → decode, PMTiles writer → sans-IO reader.

use datars_geo::mvt::{Feature, GeomType, Layer, Value, VectorTile};
use datars_geo::pmtiles::{self, ByteRange, Compression, Reader, Step, TileType, Writer, WriterOptions};
use datars_geo::tile::{self, tiles_covering};
use datars_geo::{GeoBbox, GeoError, TileId};
use datars_math::Vec2;

/// An in-memory "remote" archive that counts range requests, like an HTTP client would.
struct Remote {
    bytes: Vec<u8>,
    fetches: Vec<ByteRange>,
}

impl Remote {
    fn new(bytes: Vec<u8>) -> Remote {
        Remote { bytes, fetches: Vec::new() }
    }

    fn fetch(&mut self, r: ByteRange) -> Result<Vec<u8>, GeoError> {
        self.fetches.push(r);
        let end = (r.offset + r.length) as usize;
        self.bytes.get(r.offset as usize..end).map(<[u8]>::to_vec).ok_or_else(|| GeoError::Format("out of range".into()))
    }

    fn open(&mut self) -> Reader {
        let n = (pmtiles::PREFIX_LEN as usize).min(self.bytes.len());
        Reader::open(&self.bytes[..n]).expect("open")
    }

    /// Drive plan_get/provide by hand, returning the tile and the steps taken.
    fn get(&mut self, reader: &mut Reader, tile: TileId) -> (Option<Vec<u8>>, Vec<ByteRange>) {
        let mut asked = Vec::new();
        for _ in 0..8 {
            match reader.plan_get(tile).expect("plan") {
                Step::Absent => return (None, asked),
                Step::Ready(b) => return (Some(b), asked),
                Step::Needs(r) => {
                    asked.push(r);
                    let b = self.fetch(r).expect("fetch");
                    reader.provide(r, &b).expect("provide");
                }
            }
        }
        panic!("did not converge");
    }
}

fn mvt_tile(t: TileId) -> Vec<u8> {
    let mut layer = Layer::new("cells");
    layer.features.push(Feature {
        id: Some(t.pmtiles_id()),
        geom_type: GeomType::Polygon,
        geometry: vec![vec![(0, 0), (4096, 0), (4096, 4096), (0, 4096)]],
        properties: vec![("z".into(), Value::Uint(t.z as u64)), ("name".into(), Value::String(format!("{}/{}/{}", t.z, t.x, t.y)))],
    });
    VectorTile { layers: vec![layer] }.encode()
}

#[test]
fn web_mercator_and_tiles_agree() {
    for &(lon, lat) in &[(18.0686, 59.3293), (-74.006, 40.7128), (151.2093, -33.8688), (0.0, 0.0), (-179.9, -80.0)] {
        let p = Vec2::new(lon, lat);
        let w = tile::lonlat_to_world(p);
        let back = tile::world_to_lonlat(w);
        assert!((back.x - lon).abs() < 1e-9 && (back.y - lat).abs() < 1e-9);
        for z in [0u8, 3, 9, 16] {
            let t = TileId::from_lonlat(p, z);
            let b = t.lonlat_bounds();
            assert!(b.west <= lon && lon <= b.east && b.south <= lat && lat <= b.north, "{t:?} {b:?}");
            assert_eq!(tiles_covering(&GeoBbox::new(lon, lat, lon, lat), z), vec![t]);
            assert_eq!(TileId::from_pmtiles_id(t.pmtiles_id()), Some(t));
        }
    }
    assert_eq!(TileId::from_lonlat(Vec2::new(18.0686, 59.3293), 4), TileId::new(4, 8, 4));
}

#[test]
fn mvt_round_trip_through_bytes() {
    let t = TileId::new(5, 17, 9);
    let bytes = mvt_tile(t);
    let tile = VectorTile::decode(&bytes).unwrap();
    let f = &tile.layer("cells").unwrap().features[0];
    assert_eq!(f.id, Some(t.pmtiles_id()));
    assert_eq!(f.get("name").and_then(Value::as_str), Some("5/17/9"));
    assert_eq!(f.polygons().len(), 1);
    assert_eq!(tile.encode(), bytes);
}

#[test]
fn pmtiles_round_trip_via_range_reads() {
    let mut w = Writer::new(WriterOptions {
        metadata: serde_json::json!({ "name": "test", "vector_layers": [{ "id": "cells" }] }),
        bounds: GeoBbox::new(10.0, 55.0, 25.0, 70.0),
        center: Vec2::new(18.0, 59.0),
        ..WriterOptions::default()
    });
    let mut tiles = Vec::new();
    for z in 0..=4u8 {
        let n = 1u32 << z;
        for x in 0..n {
            for y in 0..n {
                let t = TileId::new(z, x, y);
                tiles.push(t);
                w.add_tile(t, mvt_tile(t));
            }
        }
    }
    assert_eq!(w.len(), tiles.len());
    let mut remote = Remote::new(w.finish().unwrap());
    let mut reader = remote.open();
    let h = reader.header().clone();
    assert_eq!((h.min_zoom, h.max_zoom, h.center_zoom), (0, 4, 0));
    assert_eq!(h.addressed_tiles, tiles.len() as u64);
    assert_eq!(h.tile_contents, tiles.len() as u64);
    assert_eq!(h.tile_type, TileType::Mvt);
    assert_eq!(h.tile_compression, Compression::Gzip);
    assert_eq!(h.leaf_length, 0, "a small archive keeps a root-only directory");
    assert!((h.bounds.west - 10.0).abs() < 1e-7 && (h.center.y - 59.0).abs() < 1e-7);

    for &t in &tiles {
        let (bytes, asked) = remote.get(&mut reader, t);
        assert_eq!(bytes.as_deref(), Some(mvt_tile(t).as_slice()), "{t:?}");
        assert_eq!(asked.len(), 1, "one range read per tile");
        assert!(asked[0].offset >= h.data_offset);
    }
    assert_eq!(remote.fetches.len(), tiles.len());
    assert_eq!(remote.get(&mut reader, TileId::new(5, 0, 0)).0, None, "beyond max zoom");
    assert_eq!(reader.plan_get(TileId::new(2, 9, 9)), Ok(Step::Absent), "invalid tile");

    let meta = remote.fetch(reader.metadata_range()).unwrap();
    let meta = reader.parse_metadata(&meta).unwrap();
    assert_eq!(meta["name"], "test");
    assert_eq!(meta["vector_layers"][0]["id"], "cells");

    // The synchronous driver agrees.
    let bytes = remote.bytes.clone();
    let got = reader.get_with(TileId::new(3, 5, 2), |r| remote.fetch(r)).unwrap();
    assert_eq!(got, Some(mvt_tile(TileId::new(3, 5, 2))));
    assert_eq!(pmtiles::read_tile(&bytes, TileId::new(4, 15, 0)).unwrap(), Some(mvt_tile(TileId::new(4, 15, 0))));
}

#[test]
fn identical_tiles_are_stored_once() {
    let ocean = mvt_tile(TileId::new(0, 0, 0));
    let mut w = Writer::new(WriterOptions::default());
    for x in 0..16 {
        for y in 0..16 {
            w.add_tile(TileId::new(4, x, y), ocean.clone());
        }
    }
    // One distinct tile in the middle breaks the run in two.
    w.add_tile(TileId::new(4, 7, 9), b"land".to_vec());
    let bytes = w.finish().unwrap();
    let r = Reader::open(&bytes).unwrap();
    let h = r.header();
    assert_eq!(h.addressed_tiles, 256);
    assert_eq!(h.tile_contents, 2);
    assert!(h.tile_entries <= 3, "runs collapse identical neighbours: {} entries", h.tile_entries);
    assert!((bytes.len() as u64) < 2000, "256 tiles in {} bytes", bytes.len());
    for &(x, y) in &[(0, 0), (15, 15), (3, 12), (7, 8)] {
        assert_eq!(pmtiles::read_tile(&bytes, TileId::new(4, x, y)).unwrap().as_deref(), Some(ocean.as_slice()));
    }
    assert_eq!(pmtiles::read_tile(&bytes, TileId::new(4, 7, 9)).unwrap().as_deref(), Some(&b"land"[..]));
    assert_eq!(pmtiles::read_tile(&bytes, TileId::new(3, 0, 0)).unwrap(), None);
}

#[test]
fn leaf_directories_resolve() {
    let mut w = Writer::new(WriterOptions { leaf_entries: Some(8), ..WriterOptions::default() });
    let mut tiles = Vec::new();
    for x in 0..20u32 {
        for y in 0..15u32 {
            let t = TileId::new(9, 270 + x, 140 + y);
            tiles.push(t);
            w.add_tile(t, format!("tile {x} {y}").into_bytes());
        }
    }
    assert_eq!(tiles.len(), 300);
    let mut remote = Remote::new(w.finish().unwrap());
    let mut reader = remote.open();
    let h = reader.header().clone();
    assert!(h.leaf_length > 0, "expected leaf directories");
    assert_eq!(h.tile_entries, 300);

    // Cold: the first lookup asks for a leaf directory, then the tile.
    let (bytes, asked) = remote.get(&mut reader, tiles[0]);
    assert_eq!(bytes, Some(b"tile 0 0".to_vec()));
    assert_eq!(asked.len(), 2);
    assert!(asked[0].offset >= h.leaf_offset && asked[0].offset + asked[0].length <= h.leaf_offset + h.leaf_length);
    assert!(asked[1].offset >= h.data_offset);

    for (i, &t) in tiles.iter().enumerate() {
        let (x, y) = (i / 15, i % 15);
        assert_eq!(remote.get(&mut reader, t).0, Some(format!("tile {x} {y}").into_bytes()), "{t:?}");
    }
    // 300 tiles + at most one fetch per leaf (300 / 8 → 38 leaves).
    assert!(remote.fetches.len() <= 300 + 38 + 1, "{} fetches", remote.fetches.len());
    assert_eq!(remote.get(&mut reader, TileId::new(9, 0, 0)).0, None);
}

#[test]
fn uncompressed_archives() {
    let mut w = Writer::new(WriterOptions { tile_compression: Compression::None, tile_type: TileType::Png, ..WriterOptions::default() });
    w.add_tile(TileId::new(2, 1, 1), vec![0x89, b'P', b'N', b'G']);
    let bytes = w.finish().unwrap();
    let r = Reader::open(&bytes).unwrap();
    assert_eq!(r.header().tile_compression, Compression::None);
    assert_eq!(r.header().tile_type, TileType::Png);
    // Stored verbatim.
    assert!(bytes.windows(4).any(|x| x == [0x89, b'P', b'N', b'G']));
    assert_eq!(pmtiles::read_tile(&bytes, TileId::new(2, 1, 1)).unwrap(), Some(vec![0x89, b'P', b'N', b'G']));
}

#[test]
fn bad_input_is_an_error() {
    assert!(Reader::open(b"not a pmtiles archive at all").is_err());
    let mut w = Writer::new(WriterOptions::default());
    w.add_tile(TileId::new(0, 0, 0), b"x".to_vec());
    let bytes = w.finish().unwrap();
    // Prefix too short for the root directory.
    assert!(Reader::open(&bytes[..pmtiles::HEADER_LEN + 2]).is_err());
    let mut reader = Reader::open(&bytes).unwrap();
    let Step::Needs(r) = reader.plan_get(TileId::new(0, 0, 0)).unwrap() else { panic!("expected a range") };
    // Wrong length, and a range that isn't tile data or a leaf.
    assert!(reader.provide(r, &vec![0u8; r.length as usize + 1]).is_err());
    assert!(reader.provide(ByteRange::new(0, 4), b"PMTi").is_err());
    // Corrupt gzip in the tile payload surfaces as a compression error.
    reader.provide(r, &vec![0u8; r.length as usize]).unwrap();
    assert!(matches!(reader.plan_get(TileId::new(0, 0, 0)), Err(GeoError::Compression(_))));
    // Truncated archives never panic.
    for cut in (0..bytes.len()).step_by(7) {
        let _ = pmtiles::read_tile(&bytes[..cut], TileId::new(0, 0, 0));
    }
}

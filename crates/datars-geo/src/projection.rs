//! Map projections: forward and inverse, deterministic (`datars_math::m` only).
//!
//! A `Projection` is plain, serializable data (it lives in the IR as `Coord::Geo { projection }`):
//! a `kind` plus d3-geo's parameters — `scale`, `translate`, `center`, `rotate`, and optional
//! small-circle (`clip_angle`) and rectangle (`clip_extent`) clipping. Projected space is y-down
//! (screen-like) for every kind:
//!
//! ```text
//! lon/lat ─rotate─► λ,φ ─raw─► x,y (north up) ─► translate + scale·(x − cx, −(y − cy))
//! ```
//!
//! where `(cx, cy)` is the raw projection of `center` (given in the rotated frame, as in d3).
//! Constructors use d3's defaults (translate `[480, 250]`, per-projection scales), so d3 examples
//! carry over; `fit_extent` computes scale and translate for a target rectangle.

use crate::clip::sphere::SphereClip;
use crate::geometry::{FeatureCollection, GeoBbox, Geometry};
use crate::sphere::{self, Rotation, EPS, EPS2, HALF_PI, PI, RAD};
use datars_math::{m, Rect, Vec2};
use serde::{Deserialize, Serialize};

/// Web Mercator's latitude limit in radians (`atan(sinh π)`).
const MERCATOR_MAX_PHI: f64 = 1.484_422_229_745_332_4;

/// Ellipsoidal transverse Mercator (Gauss–Krüger), the basis of UTM and national grids such as
/// SWEREF 99 TM. Uses Krüger's n-series to 4th order (Lantmäteriet's formulation): millimetre
/// accuracy within a zone.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransverseMercator {
    /// Central meridian λ₀ in degrees.
    pub central_meridian: f64,
    /// Scale factor on the central meridian (0.9996 for UTM).
    pub scale_factor: f64,
    pub false_easting: f64,
    pub false_northing: f64,
    /// Ellipsoid semi-major axis in metres.
    pub semi_major: f64,
    /// Ellipsoid flattening.
    pub flattening: f64,
}

impl TransverseMercator {
    /// UTM zone `zone` (1..=60) on WGS 84; `north = false` adds the 10 000 km false northing.
    pub fn utm(zone: u8, north: bool) -> TransverseMercator {
        let zone = zone.clamp(1, 60) as f64;
        TransverseMercator {
            central_meridian: -183.0 + 6.0 * zone,
            scale_factor: 0.9996,
            false_easting: 500_000.0,
            false_northing: if north { 0.0 } else { 10_000_000.0 },
            semi_major: 6_378_137.0,
            flattening: 1.0 / 298.257_223_563,
        }
    }

    /// SWEREF 99 TM (EPSG:3006), Sweden's national grid: GRS 80, λ₀ = 15°E, k₀ = 0.9996.
    pub fn sweref99tm() -> TransverseMercator {
        TransverseMercator {
            central_meridian: 15.0,
            scale_factor: 0.9996,
            false_easting: 500_000.0,
            false_northing: 0.0,
            semi_major: 6_378_137.0,
            flattening: 1.0 / 298.257_222_101,
        }
    }

    /// lon/lat (degrees) → grid (easting, northing) in metres.
    pub fn grid_forward(&self, lonlat: Vec2) -> Vec2 {
        let c = TmConsts::new(self);
        let dl = sphere::wrap_lambda((lonlat.x - self.central_meridian) * RAD);
        let (e, n) = c.forward(dl, lonlat.y * RAD);
        Vec2::new(self.false_easting + e, self.false_northing + n)
    }

    /// Grid (easting, northing) in metres → lon/lat (degrees).
    pub fn grid_inverse(&self, en: Vec2) -> Vec2 {
        let c = TmConsts::new(self);
        let (dl, phi) = c.inverse(en.x - self.false_easting, en.y - self.false_northing);
        Vec2::new(sphere::wrap_lambda(dl + self.central_meridian * RAD) / RAD, phi / RAD)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProjectionKind {
    Equirectangular,
    /// Spherical Mercator, latitudes clamped to ±85.0511°.
    Mercator,
    /// Mercator with the web-map conventions; see `Projection::web_mercator` (world 0..1) and
    /// `Projection::web_mercator_px` (512·2^z pixel space).
    WebMercator,
    EqualEarth,
    NaturalEarth1,
    /// Albers equal-area conic with two standard parallels (degrees).
    AlbersConic { parallels: [f64; 2] },
    /// The US composite: lower 48 plus Alaska and Hawaii insets (d3's `geoAlbersUsa`).
    AlbersUsa,
    /// Lambert conformal conic with two standard parallels (degrees).
    LambertConformalConic { parallels: [f64; 2] },
    /// A globe seen from infinitely far away; clips to the visible hemisphere.
    Orthographic,
    /// Ellipsoidal transverse Mercator; raw units are metres.
    TransverseMercator(TransverseMercator),
    /// No projection: coordinates are already planar (floor plans, fictional maps). With
    /// `y_up: false` the identity; `y_up: true` flips y for data drawn with y pointing up.
    Planar { y_up: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Projection {
    pub kind: ProjectionKind,
    pub scale: f64,
    pub translate: Vec2,
    /// The point (degrees, in the rotated frame) that lands on `translate`.
    #[serde(default)]
    pub center: Vec2,
    /// Spherical rotation (λ, φ, γ) in degrees, applied before projecting.
    #[serde(default)]
    pub rotate: [f64; 3],
    /// Small-circle clip radius in degrees (orthographic defaults to the horizon, 90°).
    #[serde(default)]
    pub clip_angle: Option<f64>,
    /// Clip rectangle in projected space. (Albers USA clips each inset to its own box instead.)
    #[serde(default)]
    pub clip_extent: Option<Rect>,
}

/// What `fit_extent` fits.
#[derive(Clone, Copy, Debug)]
pub enum Fit<'a> {
    Bbox(GeoBbox),
    Geometry(&'a Geometry),
    Features(&'a FeatureCollection),
    /// The whole sphere: a globe's disc — the horizon around the view centre, for projections that
    /// clip to a small circle — else the world's outline. The same fit at any rotation, so a
    /// turning globe keeps its size (d3's `{type: "Sphere"}`).
    Sphere,
}

const D3_TRANSLATE: Vec2 = Vec2::new(480.0, 250.0);

impl Projection {
    fn with_kind(kind: ProjectionKind, scale: f64) -> Projection {
        Projection { kind, scale, translate: D3_TRANSLATE, center: Vec2::ZERO, rotate: [0.0; 3], clip_angle: None, clip_extent: None }
    }

    pub fn equirectangular() -> Projection {
        Projection::with_kind(ProjectionKind::Equirectangular, 152.63)
    }
    pub fn mercator() -> Projection {
        Projection::with_kind(ProjectionKind::Mercator, 961.0 / m::TAU)
    }
    /// Web Mercator in world units: the unit square, (0, 0) at the north-west corner, y down —
    /// the space `tile` uses.
    pub fn web_mercator() -> Projection {
        Projection { translate: Vec2::new(0.5, 0.5), ..Projection::with_kind(ProjectionKind::WebMercator, 1.0 / m::TAU) }
    }
    /// Web Mercator in pixels at `zoom`: the world is `512·2^zoom` px across.
    pub fn web_mercator_px(zoom: f64) -> Projection {
        let size = crate::tile::world_size_px(zoom);
        Projection { translate: Vec2::new(size / 2.0, size / 2.0), ..Projection::with_kind(ProjectionKind::WebMercator, size / m::TAU) }
    }
    pub fn equal_earth() -> Projection {
        Projection::with_kind(ProjectionKind::EqualEarth, 177.158)
    }
    pub fn natural_earth1() -> Projection {
        Projection::with_kind(ProjectionKind::NaturalEarth1, 175.295)
    }
    /// Albers equal-area conic with the given standard parallels (d3 `geoConicEqualArea`).
    pub fn albers_conic(parallels: [f64; 2]) -> Projection {
        Projection { center: Vec2::new(0.0, 33.6442), ..Projection::with_kind(ProjectionKind::AlbersConic { parallels }, 155.424) }
    }
    /// d3's `geoAlbers`: the conterminous-US preset of the Albers conic.
    pub fn albers() -> Projection {
        Projection {
            rotate: [96.0, 0.0, 0.0],
            center: Vec2::new(-0.6, 38.7),
            ..Projection::with_kind(ProjectionKind::AlbersConic { parallels: [29.5, 45.5] }, 1070.0)
        }
    }
    /// d3's `geoAlbersUsa`: lower 48 with Alaska and Hawaii insets.
    pub fn albers_usa() -> Projection {
        Projection::with_kind(ProjectionKind::AlbersUsa, 1070.0)
    }
    pub fn lambert_conformal_conic(parallels: [f64; 2]) -> Projection {
        Projection::with_kind(ProjectionKind::LambertConformalConic { parallels }, 109.5)
    }
    /// A globe centred on (lon, lat).
    pub fn orthographic(center_lon: f64, center_lat: f64) -> Projection {
        Projection { rotate: [-center_lon, -center_lat, 0.0], ..Projection::with_kind(ProjectionKind::Orthographic, 249.5) }
    }
    /// Transverse Mercator in grid metres: scale 1 and translate `(FE, −FN)`, so projected space is
    /// `(easting, −northing)` (y down like every projection). `TransverseMercator::grid_forward`
    /// gives (easting, northing) directly.
    pub fn transverse_mercator(tm: TransverseMercator) -> Projection {
        Projection {
            translate: Vec2::new(tm.false_easting, -tm.false_northing),
            ..Projection::with_kind(ProjectionKind::TransverseMercator(tm), 1.0)
        }
    }
    /// UTM zone (WGS 84).
    pub fn utm(zone: u8, north: bool) -> Projection {
        Projection::transverse_mercator(TransverseMercator::utm(zone, north))
    }
    /// SWEREF 99 TM (EPSG:3006).
    pub fn sweref99tm() -> Projection {
        Projection::transverse_mercator(TransverseMercator::sweref99tm())
    }
    /// Identity: planar coordinates pass through (scale 1, translate 0, y down).
    pub fn planar() -> Projection {
        Projection { translate: Vec2::ZERO, ..Projection::with_kind(ProjectionKind::Planar { y_up: false }, 1.0) }
    }
    /// Planar data whose y axis points up (CAD drawings, fictional maps with north up).
    pub fn planar_y_up() -> Projection {
        Projection { translate: Vec2::ZERO, ..Projection::with_kind(ProjectionKind::Planar { y_up: true }, 1.0) }
    }

    pub fn with_scale(mut self, k: f64) -> Projection {
        self.scale = k;
        self
    }
    pub fn with_translate(mut self, t: Vec2) -> Projection {
        self.translate = t;
        self
    }
    pub fn with_center(mut self, c: Vec2) -> Projection {
        self.center = c;
        self
    }
    pub fn with_rotate(mut self, r: [f64; 3]) -> Projection {
        self.rotate = r;
        self
    }
    pub fn with_clip_angle(mut self, deg: Option<f64>) -> Projection {
        self.clip_angle = deg;
        self
    }
    pub fn with_clip_extent(mut self, r: Option<Rect>) -> Projection {
        self.clip_extent = r;
        self
    }

    pub fn is_planar(&self) -> bool {
        matches!(self.kind, ProjectionKind::Planar { .. })
    }

    /// Project a point. `None` when the projection clips it (the far side of a globe, outside the
    /// Albers USA insets, outside `clip_extent`) or it has no finite image. For many points, use
    /// `projector()` once instead.
    pub fn forward(&self, lonlat: Vec2) -> Option<Vec2> {
        self.projector().forward(lonlat)
    }

    /// Unproject a point. `None` outside the projection's domain (e.g. off the globe's disc).
    pub fn inverse(&self, p: Vec2) -> Option<Vec2> {
        self.projector().inverse(p)
    }

    /// The projection compiled for repeated use (constants, rotation and clipping resolved once).
    pub fn projector(&self) -> Projector {
        let usa = matches!(self.kind, ProjectionKind::AlbersUsa).then_some((self.scale, self.translate));
        Projector { parts: self.parts(), usa }
    }

    /// Fit scale and translate so `what` fills `extent` (centred, aspect kept) — d3's `fitExtent`.
    pub fn fit_extent(&self, what: Fit<'_>, extent: Rect) -> Projection {
        fit_extent(self, what, extent)
    }

    /// `fit_extent` into `(0, 0, width, height)`.
    pub fn fit_size(&self, what: Fit<'_>, width: f64, height: f64) -> Projection {
        fit_extent(self, what, Rect::new(0.0, 0.0, width, height))
    }

    /// The affine map from a Web-Mercator tile's local coordinates (`0..extent` across the tile,
    /// y down) into this projection's space — `Some` for the Mercator kinds without rotation, where
    /// the map is exactly a uniform scale plus a translation. Vector tiles can then keep their
    /// geometry in tile units (decoded and batched once) and be placed by a transform; every other
    /// projection reprojects vertices instead (`None`).
    pub fn tile_transform(&self, tile: crate::tile::TileId, extent: f64) -> Option<datars_math::Affine> {
        if !matches!(self.kind, ProjectionKind::Mercator | ProjectionKind::WebMercator) || self.rotate != [0.0; 3] || extent.is_nan() || extent <= 0.0 {
            return None;
        }
        // Raw Mercator of the centre (the projection's (cx, cy)); λ = 2π·wx − π, y = π·(1 − 2·wy).
        let cx = self.center.x * RAD;
        let cy = m::ln(m::tan((HALF_PI + (self.center.y * RAD).clamp(-MERCATOR_MAX_PHI, MERCATOR_MAX_PHI)) / 2.0));
        let k = self.scale;
        let n = (1u64 << tile.z.min(62)) as f64;
        let s = m::TAU * k / (n * extent);
        let e = self.translate.x - k * (PI + cx) + m::TAU * k * tile.x as f64 / n;
        let f = self.translate.y + k * (cy - PI) + m::TAU * k * tile.y as f64 / n;
        Some(datars_math::Affine([s, 0.0, 0.0, s, e, f]))
    }

    /// The concrete projections this one is made of: one, or three for Albers USA.
    pub(crate) fn parts(&self) -> Vec<Compiled> {
        match &self.kind {
            ProjectionKind::AlbersUsa => albers_usa_parts(self.scale, self.translate),
            _ => vec![Compiled::new(self)],
        }
    }
}

impl datars_math::StableHash for Projection {
    /// Hashes the canonical JSON form (shortest round-trip floats), so equal projections hash
    /// equally on every target — a cache key for projected geometry.
    fn stable_hash(&self, h: &mut datars_math::Hash64) {
        h.str(&serde_json::to_string(self).unwrap_or_default());
    }
}

/// A compiled projection: `Projection::projector`. Cheap to call per point (dot maps, per-frame
/// anchors); build it once per projection change.
#[derive(Clone, Debug)]
pub struct Projector {
    parts: Vec<Compiled>,
    /// Albers USA's scale and translate, for its inset-picking inverse.
    usa: Option<(f64, Vec2)>,
}

impl Projector {
    /// See `Projection::forward`.
    pub fn forward(&self, lonlat: Vec2) -> Option<Vec2> {
        self.parts.iter().find_map(|p| p.forward(lonlat))
    }

    /// See `Projection::inverse`. For Albers USA the inset is chosen by position, as in d3.
    pub fn inverse(&self, p: Vec2) -> Option<Vec2> {
        let i = match self.usa {
            Some((k, t)) => {
                let (x, y) = ((p.x - t.x) / k, (p.y - t.y) / k);
                if (0.120..0.234).contains(&y) && (-0.425..-0.214).contains(&x) {
                    1
                } else if (0.166..0.234).contains(&y) && (-0.214..-0.115).contains(&x) {
                    2
                } else {
                    0
                }
            }
            None => 0,
        };
        self.parts.get(i)?.inverse(p)
    }
}

/// d3's Albers USA: three conic equal-area projections with fixed insets, each clipped to its box.
fn albers_usa_parts(k: f64, t: Vec2) -> Vec<Compiled> {
    let (x, y) = (t.x, t.y);
    let e = EPS;
    let ext = |x0: f64, y0: f64, x1: f64, y1: f64| Some(Rect::new(x0, y0, x1 - x0, y1 - y0));
    let lower48 = Projection {
        scale: k,
        translate: t,
        clip_extent: ext(x - 0.455 * k, y - 0.238 * k, x + 0.455 * k, y + 0.238 * k),
        ..Projection::albers()
    };
    let alaska = Projection {
        kind: ProjectionKind::AlbersConic { parallels: [55.0, 65.0] },
        rotate: [154.0, 0.0, 0.0],
        center: Vec2::new(-2.0, 58.5),
        scale: 0.35 * k,
        translate: Vec2::new(x - 0.307 * k, y + 0.201 * k),
        clip_extent: ext(x - 0.425 * k + e, y + 0.120 * k + e, x - 0.214 * k - e, y + 0.234 * k - e),
        clip_angle: None,
    };
    let hawaii = Projection {
        kind: ProjectionKind::AlbersConic { parallels: [8.0, 18.0] },
        rotate: [157.0, 0.0, 0.0],
        center: Vec2::new(-3.0, 19.9),
        scale: k,
        translate: Vec2::new(x - 0.205 * k, y + 0.212 * k),
        clip_extent: ext(x - 0.214 * k + e, y + 0.166 * k + e, x - 0.115 * k - e, y + 0.234 * k - e),
        clip_angle: None,
    };
    vec![Compiled::new(&lower48), Compiled::new(&alaska), Compiled::new(&hawaii)]
}

/// Fit a projection to a rectangle (see `Projection::fit_extent`). Projects at a reference scale
/// through the full pipeline (clipping and resampling included), then solves for scale and
/// translate, which enter linearly.
pub fn fit_extent(projection: &Projection, what: Fit<'_>, extent: Rect) -> Projection {
    const K0: f64 = 150.0;
    let reference = Projection { scale: K0, translate: Vec2::ZERO, clip_extent: None, ..projection.clone() };
    let mut b = Rect::empty();
    let mut add = |g: &Geometry| {
        if let Some(r) = crate::project::project(g, &reference, 0.5).bounds() {
            b = b.union(&r);
        }
    };
    match what {
        Fit::Bbox(bb) => add(&bb.to_polygon(1.0)),
        Fit::Geometry(g) => add(g),
        Fit::Features(fc) => fc.features.iter().for_each(|f| add(&f.geometry)),
        Fit::Sphere => match horizon(projection) {
            Some(radius) => {
                for p in small_circle(Vec2::new(-projection.rotate[0], -projection.rotate[1]), radius) {
                    if let Some(q) = reference.forward(p) {
                        b = b.union(&Rect::new(q.x, q.y, 0.0, 0.0));
                    }
                }
            }
            None => add(&GeoBbox::new(-180.0, -90.0, 180.0, 90.0).to_polygon(1.0)),
        },
    }
    if b.is_empty() {
        return projection.clone();
    }
    let (w, h) = (extent.w, extent.h);
    let kx = if b.w > 0.0 { w / b.w } else { f64::INFINITY };
    let ky = if b.h > 0.0 { h / b.h } else { f64::INFINITY };
    let k = kx.min(ky);
    let mut out = projection.clone();
    if !k.is_finite() {
        // A single point: keep the scale, centre it.
        let c = b.center() * (projection.scale / K0);
        out.translate = extent.center() - c;
        return out;
    }
    out.scale = K0 * k;
    out.translate = Vec2::new(extent.x + (w - k * (b.x + b.x1())) / 2.0, extent.y + (h - k * (b.y + b.y1())) / 2.0);
    out
}

/// The angular radius (degrees) a projection clips the sphere to, when it clips to a small circle
/// around its centre (a globe), just inside the edge so every sample projects.
fn horizon(p: &Projection) -> Option<f64> {
    match (p.clip_angle, &p.kind) {
        (Some(a), _) => Some(a - 1e-6),
        (None, ProjectionKind::Orthographic) => Some(90.0 - 1e-6),
        _ => None,
    }
}

/// Points (lon, lat in degrees) every 2° of bearing on the small circle of `radius` degrees around
/// `center` — the spherical destination formula.
fn small_circle(center: Vec2, radius: f64) -> impl Iterator<Item = Vec2> {
    let (l0, p0, d) = (center.x * RAD, center.y * RAD, radius * RAD);
    (0..180).map(move |i| {
        let b = i as f64 * 2.0 * RAD;
        let lat = m::asin(m::sin(p0) * m::cos(d) + m::cos(p0) * m::sin(d) * m::cos(b));
        let lon = l0 + m::atan2(m::sin(b) * m::sin(d) * m::cos(p0), m::cos(d) - m::sin(p0) * m::sin(lat));
        Vec2::new(lon / RAD, lat / RAD)
    })
}

// ---- raw projections -----------------------------------------------------------------------

/// A raw projection on the unit sphere (or ellipsoid metres for TM): radians in, north-up plane
/// out, with precomputed constants.
#[derive(Clone, Debug)]
pub(crate) enum Raw {
    Equirectangular,
    Mercator,
    EqualEarth,
    NaturalEarth1,
    ConicEqualArea { n: f64, c: f64, r0: f64 },
    CylindricalEqualArea { cos_phi0: f64 },
    ConicConformal { n: f64, f: f64 },
    Orthographic,
    TransverseMercator(TmConsts),
    Planar { y_up: bool },
}

const EE_A1: f64 = 1.340_264;
const EE_A2: f64 = -0.081_106;
const EE_A3: f64 = 0.000_893;
const EE_A4: f64 = 0.003_796;

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn tany(y: f64) -> f64 {
    m::tan((HALF_PI + y) / 2.0)
}

fn atanh(x: f64) -> f64 {
    let x = x.clamp(-1.0 + 1e-16, 1.0 - 1e-16);
    0.5 * m::ln((1.0 + x) / (1.0 - x))
}

impl Raw {
    fn conic_equal_area(p0: f64, p1: f64) -> Raw {
        let sy0 = m::sin(p0);
        let n = (sy0 + m::sin(p1)) / 2.0;
        if n.abs() < EPS {
            return Raw::CylindricalEqualArea { cos_phi0: m::cos(p0) };
        }
        let c = 1.0 + sy0 * (2.0 * n - sy0);
        Raw::ConicEqualArea { n, c, r0: c.sqrt() / n }
    }

    fn conic_conformal(p0: f64, p1: f64) -> Raw {
        let cy0 = m::cos(p0);
        let n = if p0 == p1 { m::sin(p0) } else { m::ln(cy0 / m::cos(p1)) / m::ln(tany(p1) / tany(p0)) };
        if n == 0.0 || !n.is_finite() {
            return Raw::Mercator;
        }
        let f = cy0 * m::pow(tany(p0), n) / n;
        Raw::ConicConformal { n, f }
    }

    pub fn forward(&self, l: f64, p: f64) -> (f64, f64) {
        match *self {
            Raw::Equirectangular => (l, p),
            Raw::Mercator => {
                let p = p.clamp(-MERCATOR_MAX_PHI, MERCATOR_MAX_PHI);
                (l, m::ln(m::tan((HALF_PI + p) / 2.0)))
            }
            Raw::EqualEarth => {
                let mm = 3f64.sqrt() / 2.0;
                let t = sphere::asin(mm * m::sin(p));
                let (t2, t6) = (t * t, t * t * t * t * t * t);
                (
                    l * m::cos(t) / (mm * (EE_A1 + 3.0 * EE_A2 * t2 + t6 * (7.0 * EE_A3 + 9.0 * EE_A4 * t2))),
                    t * (EE_A1 + EE_A2 * t2 + t6 * (EE_A3 + EE_A4 * t2)),
                )
            }
            Raw::NaturalEarth1 => {
                let (p2, p4) = (p * p, p * p * p * p);
                (
                    l * (0.8707 - 0.131_979 * p2 + p4 * (-0.013_791 + p4 * (0.003_971 * p2 - 0.001_529 * p4))),
                    p * (1.007_226 + p2 * (0.015_085 + p4 * (-0.044_475 + 0.028_874 * p2 - 0.005_916 * p4))),
                )
            }
            Raw::ConicEqualArea { n, c, r0 } => {
                let r = (c - 2.0 * n * m::sin(p)).max(0.0).sqrt() / n;
                let x = l * n;
                (r * m::sin(x), r0 - r * m::cos(x))
            }
            Raw::CylindricalEqualArea { cos_phi0 } => (l * cos_phi0, m::sin(p) / cos_phi0),
            Raw::ConicConformal { n, f } => {
                let p = if f > 0.0 { p.max(-HALF_PI + EPS) } else { p.min(HALF_PI - EPS) };
                let r = f / m::pow(tany(p), n);
                (r * m::sin(n * l), f - r * m::cos(n * l))
            }
            Raw::Orthographic => (m::cos(p) * m::sin(l), m::sin(p)),
            Raw::TransverseMercator(ref c) => c.forward(l, p),
            Raw::Planar { y_up } => (l, if y_up { p } else { -p }),
        }
    }

    pub fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let r = match *self {
            Raw::Equirectangular => (x, y),
            Raw::Mercator => (x, 2.0 * m::atan(m::exp(y)) - HALF_PI),
            Raw::EqualEarth => {
                let mm = 3f64.sqrt() / 2.0;
                let mut t = y;
                let (mut t2, mut t6) = (t * t, t * t * t * t * t * t);
                for _ in 0..12 {
                    let fy = t * (EE_A1 + EE_A2 * t2 + t6 * (EE_A3 + EE_A4 * t2)) - y;
                    let fpy = EE_A1 + 3.0 * EE_A2 * t2 + t6 * (7.0 * EE_A3 + 9.0 * EE_A4 * t2);
                    let delta = fy / fpy;
                    t -= delta;
                    t2 = t * t;
                    t6 = t2 * t2 * t2;
                    if delta.abs() < EPS2 {
                        break;
                    }
                }
                (mm * x * (EE_A1 + 3.0 * EE_A2 * t2 + t6 * (7.0 * EE_A3 + 9.0 * EE_A4 * t2)) / m::cos(t), sphere::asin(m::sin(t) / mm))
            }
            Raw::NaturalEarth1 => {
                let mut p = y;
                for _ in 0..25 {
                    let (p2, p4) = (p * p, p * p * p * p);
                    let delta = (p * (1.007_226 + p2 * (0.015_085 + p4 * (-0.044_475 + 0.028_874 * p2 - 0.005_916 * p4))) - y)
                        / (1.007_226 + p2 * (0.015_085 * 3.0 + p4 * (-0.044_475 * 7.0 + 0.028_874 * 9.0 * p2 - 0.005_916 * 11.0 * p4)));
                    p -= delta;
                    if delta.abs() <= EPS2 {
                        break;
                    }
                }
                let p2 = p * p;
                (x / (0.8707 + p2 * (-0.131_979 + p2 * (-0.013_791 + p2 * p2 * p2 * (0.003_971 - 0.001_529 * p2)))), p)
            }
            Raw::ConicEqualArea { n, c, r0 } => {
                let r0y = r0 - y;
                let mut l = m::atan2(x, r0y.abs()) * sign(r0y);
                if r0y * n < 0.0 {
                    l -= PI * sign(x) * sign(r0y);
                }
                (l / n, sphere::asin((c - (x * x + r0y * r0y) * n * n) / (2.0 * n)))
            }
            Raw::CylindricalEqualArea { cos_phi0 } => (x / cos_phi0, sphere::asin(y * cos_phi0)),
            Raw::ConicConformal { n, f } => {
                let fy = f - y;
                let r = sign(n) * (x * x + fy * fy).sqrt();
                let mut l = m::atan2(x, fy.abs()) * sign(fy);
                if fy * n < 0.0 {
                    l -= PI * sign(x) * sign(fy);
                }
                (l / n, 2.0 * m::atan(m::pow(f / r, 1.0 / n)) - HALF_PI)
            }
            Raw::Orthographic => {
                let z = m::hypot(x, y);
                if z > 1.0 + 1e-9 {
                    return None;
                }
                let c = sphere::asin(z);
                let (sc, cc) = (m::sin(c), m::cos(c));
                (m::atan2(x * sc, z * cc), sphere::asin(if z != 0.0 { y * sc / z } else { z }))
            }
            Raw::TransverseMercator(ref c) => c.inverse(x, y),
            Raw::Planar { y_up } => (x, if y_up { y } else { -y }),
        };
        (r.0.is_finite() && r.1.is_finite()).then_some(r)
    }
}

/// Precomputed Gauss–Krüger series constants.
#[derive(Clone, Debug)]
pub(crate) struct TmConsts {
    k0a: f64,
    a: [f64; 4],
    beta: [f64; 4],
    delta: [f64; 4],
    astar: [f64; 4],
}

impl TmConsts {
    fn new(p: &TransverseMercator) -> TmConsts {
        let f = p.flattening;
        let e2 = f * (2.0 - f);
        let n = f / (2.0 - f);
        let (n2, n3, n4) = (n * n, n * n * n, n * n * n * n);
        let (e4, e6, e8) = (e2 * e2, e2 * e2 * e2, e2 * e2 * e2 * e2);
        let a_roof = p.semi_major / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0);
        TmConsts {
            k0a: p.scale_factor * a_roof,
            a: [e2, (5.0 * e4 - e6) / 6.0, (104.0 * e6 - 45.0 * e8) / 120.0, 1237.0 * e8 / 1260.0],
            beta: [
                n / 2.0 - 2.0 * n2 / 3.0 + 5.0 * n3 / 16.0 + 41.0 * n4 / 180.0,
                13.0 * n2 / 48.0 - 3.0 * n3 / 5.0 + 557.0 * n4 / 1440.0,
                61.0 * n3 / 240.0 - 103.0 * n4 / 140.0,
                49561.0 * n4 / 161_280.0,
            ],
            delta: [
                n / 2.0 - 2.0 * n2 / 3.0 + 37.0 * n3 / 96.0 - n4 / 360.0,
                n2 / 48.0 + n3 / 15.0 - 437.0 * n4 / 1440.0,
                17.0 * n3 / 480.0 - 37.0 * n4 / 840.0,
                4397.0 * n4 / 161_280.0,
            ],
            astar: [
                e2 + e4 + e6 + e8,
                -(7.0 * e4 + 17.0 * e6 + 30.0 * e8) / 6.0,
                (224.0 * e6 + 889.0 * e8) / 120.0,
                -(4279.0 * e8) / 1260.0,
            ],
        }
    }

    /// (Δλ, φ) radians → (easting offset, northing) metres, before false easting/northing.
    fn forward(&self, dl: f64, phi: f64) -> (f64, f64) {
        let s = m::sin(phi);
        let s2 = s * s;
        let [a, b, c, d] = self.a;
        let phi_s = phi - s * m::cos(phi) * (a + s2 * (b + s2 * (c + s2 * d)));
        let xi = m::atan2(m::sin(phi_s), m::cos(phi_s) * m::cos(dl));
        let eta = atanh(m::cos(phi_s) * m::sin(dl));
        let (mut x, mut y) = (xi, eta);
        for (j, bj) in self.beta.iter().enumerate() {
            let k = 2.0 * (j as f64 + 1.0);
            x += bj * m::sin(k * xi) * m::cosh(k * eta);
            y += bj * m::cos(k * xi) * m::sinh(k * eta);
        }
        (self.k0a * y, self.k0a * x)
    }

    fn inverse(&self, e: f64, n: f64) -> (f64, f64) {
        let (xi, eta) = (n / self.k0a, e / self.k0a);
        let (mut xi1, mut eta1) = (xi, eta);
        for (j, dj) in self.delta.iter().enumerate() {
            let k = 2.0 * (j as f64 + 1.0);
            xi1 -= dj * m::sin(k * xi) * m::cosh(k * eta);
            eta1 -= dj * m::cos(k * xi) * m::sinh(k * eta);
        }
        let phi_s = sphere::asin(m::sin(xi1) / m::cosh(eta1));
        let dl = m::atan2(m::sinh(eta1), m::cos(xi1));
        let s = m::sin(phi_s);
        let s2 = s * s;
        let [a, b, c, d] = self.astar;
        (dl, phi_s + s * m::cos(phi_s) * (a + s2 * (b + s2 * (c + s2 * d))))
    }
}

// ---- compiled ------------------------------------------------------------------------------

/// A projection ready to run: raw projection, rotation, affine and clipping resolved.
#[derive(Clone, Debug)]
pub(crate) struct Compiled {
    raw: Raw,
    rot: Rotation,
    k: f64,
    tx: f64,
    ty: f64,
    cx: f64,
    cy: f64,
    /// Degrees → radians for geographic kinds, 1 for planar.
    unit: f64,
    pub preclip: Option<SphereClip>,
    pub extent: Option<Rect>,
}

impl Compiled {
    pub fn new(p: &Projection) -> Compiled {
        let two = |ps: &[f64; 2]| (ps[0] * RAD, ps[1] * RAD);
        let mut rotate = p.rotate;
        let raw = match &p.kind {
            ProjectionKind::Equirectangular => Raw::Equirectangular,
            ProjectionKind::Mercator | ProjectionKind::WebMercator => Raw::Mercator,
            ProjectionKind::EqualEarth => Raw::EqualEarth,
            ProjectionKind::NaturalEarth1 => Raw::NaturalEarth1,
            ProjectionKind::AlbersConic { parallels } => {
                let (a, b) = two(parallels);
                Raw::conic_equal_area(a, b)
            }
            // Only reached through `parts()`, which expands the composite.
            ProjectionKind::AlbersUsa => Raw::conic_equal_area(29.5 * RAD, 45.5 * RAD),
            ProjectionKind::LambertConformalConic { parallels } => {
                let (a, b) = two(parallels);
                Raw::conic_conformal(a, b)
            }
            ProjectionKind::Orthographic => Raw::Orthographic,
            ProjectionKind::TransverseMercator(tm) => {
                rotate[0] -= tm.central_meridian; // the raw works on Δλ
                Raw::TransverseMercator(TmConsts::new(tm))
            }
            ProjectionKind::Planar { y_up } => Raw::Planar { y_up: *y_up },
        };
        let planar = matches!(raw, Raw::Planar { .. });
        let unit = if planar { 1.0 } else { RAD };
        let (cx, cy) = raw.forward(p.center.x * unit, p.center.y * unit);
        let preclip = if planar {
            None
        } else if let Some(a) = p.clip_angle {
            Some(SphereClip::Circle { radius: a * RAD })
        } else if matches!(p.kind, ProjectionKind::Orthographic) {
            Some(SphereClip::Circle { radius: (90.0 + EPS) * RAD })
        } else {
            Some(SphereClip::Antimeridian)
        };
        Compiled {
            raw,
            rot: Rotation::new(if planar { [0.0; 3] } else { rotate }),
            k: p.scale,
            tx: p.translate.x,
            ty: p.translate.y,
            cx,
            cy,
            unit,
            preclip,
            extent: p.clip_extent,
        }
    }

    pub fn is_planar(&self) -> bool {
        self.preclip.is_none()
    }

    /// Input coordinates → rotated radians (or planar units unchanged).
    #[inline]
    pub fn rotate(&self, p: Vec2) -> (f64, f64) {
        if self.is_planar() {
            (p.x, p.y)
        } else {
            self.rot.forward(p.x * self.unit, p.y * self.unit)
        }
    }

    /// Radians → rotated radians (geographic kinds).
    #[inline]
    pub fn rotate_radians(&self, l: f64, p: f64) -> (f64, f64) {
        self.rot.forward(l, p)
    }

    /// Rotated coordinates → projected space (raw projection + affine).
    #[inline]
    pub fn project_rotated(&self, l: f64, p: f64) -> Vec2 {
        let (x, y) = self.raw.forward(l, p);
        Vec2::new(self.tx + self.k * (x - self.cx), self.ty - self.k * (y - self.cy))
    }

    pub fn forward(&self, lonlat: Vec2) -> Option<Vec2> {
        let (l, p) = self.rotate(lonlat);
        if let Some(c) = &self.preclip {
            if !c.visible(l, p) {
                return None;
            }
        }
        let v = self.project_rotated(l, p);
        if !v.is_finite() {
            return None;
        }
        match self.extent {
            Some(e) if !e.contains(v) => None,
            _ => Some(v),
        }
    }

    pub fn inverse(&self, v: Vec2) -> Option<Vec2> {
        let x = (v.x - self.tx) / self.k + self.cx;
        let y = -(v.y - self.ty) / self.k + self.cy;
        let (l, p) = self.raw.inverse(x, y)?;
        if self.is_planar() {
            return Some(Vec2::new(l, p));
        }
        let (l, p) = self.rot.inverse(l, p);
        Some(Vec2::new(l / self.unit, p / self.unit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d3_default_origins() {
        let t = Vec2::new(480.0, 250.0);
        for p in [Projection::equirectangular(), Projection::mercator(), Projection::equal_earth(), Projection::natural_earth1()] {
            let v = p.forward(Vec2::ZERO).unwrap();
            assert!((v - t).len() < 1e-9, "{:?}", p.kind);
        }
        // Equirectangular: 1 radian = scale px.
        let e = Projection::equirectangular().forward(Vec2::new(180.0, 0.0)).unwrap();
        assert!((e.x - (480.0 + 152.63 * PI)).abs() < 1e-9);
    }

    #[test]
    fn tile_transform_matches_forward_for_mercator() {
        let fitted = Projection::web_mercator().fit_extent(Fit::Bbox(GeoBbox::new(-30.0, 40.0, 40.0, 70.0)), Rect::new(10.0, 10.0, 700.0, 400.0));
        let rotated = Projection::mercator().with_center(Vec2::new(18.0, 59.3)).with_scale(5000.0);
        for p in [fitted, rotated, Projection::web_mercator_px(3.0)] {
            for tile in [crate::tile::TileId::new(0, 0, 0), crate::tile::TileId::new(5, 17, 9), crate::tile::TileId::new(12, 2252, 1203)] {
                let xf = p.tile_transform(tile, 4096.0).expect("mercator is affine");
                // Buffer coordinates past the tile edge too, except where they'd leave the world
                // (forward wraps longitude and clamps latitude; the affine map extends linearly).
                for q in [(0, 0), (4096, 4096), (1000, 3000), (-64, 4160)] {
                    if tile.z == 0 && q.0 < 0 {
                        continue;
                    }
                    let want = p.forward(crate::mvt::local_to_lonlat(q, tile, 4096)).unwrap();
                    let got = xf.apply(Vec2::new(q.0 as f64, q.1 as f64));
                    let tol = 1e-9 * (1.0 + want.x.abs().max(want.y.abs()));
                    assert!((got - want).len() < tol, "{:?} {tile:?} {q:?}: {got:?} vs {want:?}", p.kind);
                }
            }
        }
        assert!(Projection::equal_earth().tile_transform(crate::tile::TileId::new(0, 0, 0), 4096.0).is_none());
        assert!(Projection::mercator().with_rotate([10.0, 0.0, 0.0]).tile_transform(crate::tile::TileId::new(0, 0, 0), 4096.0).is_none());
    }

    #[test]
    fn conic_constants_fall_back() {
        assert!(matches!(Raw::conic_equal_area(10.0 * RAD, -10.0 * RAD), Raw::CylindricalEqualArea { .. }));
        assert!(matches!(Raw::conic_conformal(0.0, 0.0), Raw::Mercator));
    }
}

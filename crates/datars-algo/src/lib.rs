//! `datars-algo` — layout algorithms as pure functions over slices.
//!
//! No tables, no scene types: inputs are slices of numbers, points and indices; outputs are
//! geometry (`datars_math::{Vec2, Rect}`, angles, extents). The standard library's chart recipes
//! (docs/15-chart-coverage.md) call these and turn the geometry into keyed scene nodes.
//!
//! Every function is:
//! * **deterministic** — no hash maps, no platform transcendentals (everything goes through
//!   `datars_math::m`), stable sorts with a total order on f64, seeded randomness only
//!   (`datars_math::Rng`), fixed iteration counts. Same input → same bits on every target (P1).
//! * **total** — empty input, zeros, NaN and ±∞ are handled (documented per function), never a
//!   panic.
//! * **indexed like its input** — output `i` belongs to input `i`, whatever order the algorithm
//!   works in internally, so keys stay attached to their geometry.
//!
//! | Family | Functions |
//! |---|---|
//! | Partitions | [`pie`], [`stack`], [`dodge`], [`waterfall`], [`funnel`], [`pareto`] |
//! | Hierarchies | [`treemap`], [`treemap_nested`], [`pack`], [`pack_values`], [`pack_hierarchy`], [`partition`], [`tree`], [`cluster`] |
//! | Flows and graphs | [`sankey`], [`force`], [`force_within`], [`chord`], [`communities`], [`community_order`], [`degrees`] |
//! | Units | [`beeswarm`], [`parliament`], [`waffle`], [`apportion`], [`calendar`] |
//! | Geometry | [`delaunay`], [`voronoi`], [`hexbin`], [`contour`], [`density`], [`scatter_in`], [`polylabel`] |
//! | Distributions | [`kde`], [`silverman_bandwidth`] (1-D Gaussian kernel density: violins, ridgelines) |
//! | Level of detail | [`pyramid`] (point sets of any size as a quadtree of density-preserving samples) |
//! | Binning | [`grid_bins`], [`bin_values`], [`mass_thresholds`] (rows counted into a grid in one pass, a value per bin, density levels by the share of rows inside) |
//! | Labels and axes | [`label_layout`], [`spread`], [`lanes`], [`nice_ticks`], [`nice_domain`], [`time_ticks`] |
//!
//! Hierarchical layouts take a parent-index slice (`&[Option<usize>]`, see [`hierarchy`]).

// Numeric kernels index several parallel arrays with one loop variable; iterator chains would
// obscure them.
#![allow(clippy::needless_range_loop)]

pub mod beeswarm;
pub mod bin2d;
pub mod business;
pub mod calendar;
pub mod chord;
pub mod community;
pub mod contour;
pub mod date;
pub mod delaunay;
pub mod density;
pub mod dodge;
pub mod force;
pub mod hexbin;
pub mod hierarchy;
pub mod kde;
pub mod label;
pub mod lanes;
pub mod pack;
pub mod parliament;
pub mod partition;
pub mod pie;
pub mod polygon;
pub mod polylabel;
mod predicates;
pub mod pyramid;
pub mod sankey;
pub mod scatter;
pub mod spread;
pub mod stack;
pub mod ticks;
pub mod tree;
pub mod treemap;
mod util;
pub mod waffle;

pub use beeswarm::{beeswarm, SwarmSide};
pub use bin2d::{bin_values, grid_bins, mass_thresholds, BinAgg, GridBins};
pub use business::{funnel, pareto, waterfall, FunnelStage};
pub use calendar::{calendar, calendar_month_outline, CalendarCell, WeekStart};
pub use chord::{chord, chord_ribbon, ChordGroup, ChordLayout, ChordRibbon};
pub use community::{communities, community_order, degrees};
pub use contour::contour;
pub use delaunay::{delaunay, voronoi, Delaunay};
pub use density::density;
pub use dodge::dodge;
pub use force::{force, force_from, force_within, ForceOptions};
pub use hexbin::{hexagon, hexbin, Hexbin};
pub use kde::{kde, silverman_bandwidth};
pub use hierarchy::{sort_by_key_desc, Hierarchy};
pub use label::{label_layout, LabelBox};
pub use lanes::lanes;
pub use pack::{enclose, pack, pack_hierarchy, pack_values, Circle, PackLayout};
pub use parliament::{parliament, ParliamentLayout};
pub use partition::partition;
pub use pie::{pie, PieSort};
pub use polygon::group_rings;
pub use polylabel::polylabel;
pub use pyramid::{pyramid, Place, PointPyramid, PyramidOptions};
pub use sankey::{sankey, LinkGeom, SankeyAlign, SankeyLayout, SankeyOptions};
pub use scatter::scatter_in;
pub use spread::spread;
pub use stack::{stack, stack_order, StackOffset, StackOrder};
pub use ticks::{nice_domain, nice_ticks, tick_step, time_tick_interval, time_ticks, time_unit_of, TimeUnit};
pub use tree::{cluster, tree, TreeOptions};
pub use treemap::{treemap, treemap_nested, TreemapOptions, GOLDEN};
pub use waffle::{apportion, waffle, WaffleOrder};

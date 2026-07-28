//! The influence field behind the frontline overlay, and its zero contour.
//!
//! Pure geometry: world-space points in, polylines in world space out. Nothing
//! here touches a canvas, a config ratio or the API — which is deliberate, since
//! a contour can be checked as numbers long before it can be checked as pixels,
//! and no amount of visual tuning rescues a field that is wrong underneath it.
//!
//! See `specs/active/frontline.md` for the model and the two rejected ones.

use crate::utils::api_definitions::foxhole::{DynamicMapData, TeamId};
use crate::utils::regions::Region;
use crate::utils::request_processing::{RenderConfig, REGION_HEIGHT, REGION_WIDTH};

/// A point in world (full-map canvas) pixels.
pub type Point = (f32, f32);

/// A traced contour, in the same space.
pub type Polyline = Vec<Point>;

/// One marching-squares crossing: the two edge points of a single cell.
type Segment = (Point, Point);

/// One structure's pull on the field, in world (full-map canvas) pixels.
///
/// World space rather than region-local space on purpose: a region's front
/// depends on the bases in the regions next to it, so computing per region
/// breaks the line at every hex seam.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    pub x: f32,
    pub y: f32,
    /// Signed, and the only place a faction is encoded: positive is Colonial,
    /// negative Warden. Magnitude is the `w` in the model, and is 1 for every
    /// footing — see [`footings`] for why they are deliberately equal, and what
    /// would change if a fortress should outweigh a lone bunker.
    pub weight: f32,
}

/// Every faction-held structure in one region, placed in world space.
///
/// **Every** structure, not just `CONTROL_ICON_TYPES` — but the raw output of
/// this is not what the field sees. [`footings`] collapses each huddle of them
/// into one point first, and that pairing is the design: keep every structure
/// as *evidence* that a side is present somewhere, then count the place once
/// rather than counting the buildings.
///
/// Narrowing to `CONTROL_ICON_TYPES` here instead would answer the same
/// question worse. It would throw away every footing that is not a town, and a
/// hex held only by field bunkers would vanish from the field entirely.
///
/// Neutral structures belong to neither side and are dropped.
pub fn sources_in(region: &Region, dynamic: &DynamicMapData, config: &RenderConfig) -> Vec<Source> {
    let (origin_x, origin_y) = config.grid_offset(region.col, region.row);

    dynamic
        .map_items
        .iter()
        .filter_map(|item| {
            let weight = match item.team_id {
                TeamId::Colonials => 1.0,
                TeamId::Wardens => -1.0,
                TeamId::None => return None,
            };

            Some(Source {
                // Pixels, never the API's normalized coordinates: REGION_WIDTH
                // and REGION_HEIGHT differ (1024 x 888), so a distance measured
                // in normalized units is an ellipse and the front comes out
                // squashed vertically.
                x: origin_x as f32 + item.x as f32 * REGION_WIDTH as f32,
                y: origin_y as f32 + item.y as f32 * REGION_HEIGHT as f32,
                weight,
            })
        })
        .collect()
}

/// Collapses each cluster of structures into a single footing.
///
/// # Why this exists
///
/// The field weighs *points*, and the API reports one point per built object.
/// A town with thirty bunker icons stacked around it therefore cast thirty
/// votes, and a lone forward base cast one — so the contour tracked where each
/// side had *built more*, not where each side *was*. Against the first live war
/// that put the line about a quarter of the way across the contested ground
/// instead of half, and it also made the line angular, because thirty points in
/// a 60 px huddle give the field a lot of local structure to follow.
///
/// Measured on a fixture with a 3:1 density asymmetry — the shape of the real
/// complaint — collapsing towns to one point each moved the contour from 138 px
/// off centre to 33 px *and* cut its total turning from 158° to 42°. Raising
/// the falloff exponent, which was the first attempt, reached only 68 px and
/// made the turning worse. This is the lever; `Model::falloff` is not.
///
/// Every footing weighs the same afterwards, which is the literal reading of
/// "centre between the footholds": a fortified town and a lone bunker are both
/// one foothold. If the line ever looks like it ignores a fortress, the
/// refinement is `weight = sqrt(count)` rather than a return to counting every
/// icon — `Source::weight` already carries a magnitude nothing reads yet.
///
/// Greedy and `O(n²)` in the worst case, and it still pays for itself many
/// times over, because the field costs `cells × sources` and this is what sets
/// `sources`. Measured in release on the real 10240 x 6216 canvas with 1995
/// structures laid out as 133 towns:
///
/// | | |
/// |---|---|
/// | clustering, 1995 → 128 footings | 0.2 ms |
/// | field over every icon | 333.9 ms |
/// | field over the footings | **21.0 ms** |
///
/// So the accuracy fix is also a 16x speedup, and the `O(n²)` never gets near
/// its worst case in practice — each pass removes everything it absorbs.
pub fn footings(sources: &[Source], radius: f32) -> Vec<Source> {
    let radius = radius.max(1.0);
    let radius_squared = radius * radius;

    let mut taken = vec![false; sources.len()];
    let mut footings = Vec::new();

    for seed in 0..sources.len() {
        if taken[seed] {
            continue;
        }

        taken[seed] = true;

        let side = sources[seed].weight;
        let (mut sum_x, mut sum_y, mut count) = (sources[seed].x, sources[seed].y, 1.0f32);

        for other in (seed + 1)..sources.len() {
            // Same side only. Two towns facing each other across a river are
            // the whole subject of the picture, and merging them would erase
            // the very boundary this is here to draw.
            if taken[other] || sources[other].weight.signum() != side.signum() {
                continue;
            }

            let dx = sources[other].x - sources[seed].x;
            let dy = sources[other].y - sources[seed].y;

            if dx * dx + dy * dy <= radius_squared {
                taken[other] = true;
                sum_x += sources[other].x;
                sum_y += sources[other].y;
                count += 1.0;
            }
        }

        footings.push(Source {
            x: sum_x / count,
            y: sum_y / count,
            weight: side.signum(),
        });
    }

    footings
}

/// The two numbers that decide the *shape* of the field, as opposed to where it
/// is sampled.
///
/// Together they answer "how much does a crowd count for?", which turned out to
/// be the whole question. See [`influence`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    /// The `ε` of `w / (d² + ε)^k`, in px². Saturates a single structure's pull
    /// at close range, so standing on top of one is not a singularity.
    pub epsilon: f32,
    /// The `k`. The field falls off as `1 / d^(2k)`.
    pub falloff: u32,
}

/// A rectangle of world space to evaluate the field over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Bounds {
    /// A region's footprint, grown by `margin` pixels on every side.
    ///
    /// The margin is not padding for its own sake: the contour has to be traced
    /// *past* the hex and masked back to the silhouette, because a polyline
    /// terminated at the hex bounds stops a stroke-width short and leaves a gap
    /// at both ends.
    pub fn around_region(region: &Region, config: &RenderConfig, margin: f32) -> Bounds {
        let (x, y) = config.grid_offset(region.col, region.row);

        Bounds {
            min_x: x as f32 - margin,
            min_y: y as f32 - margin,
            max_x: x as f32 + REGION_WIDTH as f32 + margin,
            max_y: y as f32 + REGION_HEIGHT as f32 + margin,
        }
    }

    pub fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(0.0)
    }

    pub fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(0.0)
    }
}

/// `F` sampled on a regular grid.
///
/// Coarse on purpose. The field is smooth by construction and the answer is
/// approximate by design, so sampling densely buys nothing but time — and the
/// coarseness is load-bearing in one place, see [`contour`].
#[derive(Debug, Clone)]
pub struct Field {
    origin: (f32, f32),
    spacing: f32,
    cols: usize,
    rows: usize,
    values: Vec<f32>,
}

impl Field {
    /// Evaluates `F(p) = Σ w / (d² + ε)^k` over `bounds`.
    ///
    /// `None` when one side has nothing here — which covers both degenerate
    /// cases the spec names, a hex held entirely by one faction and a hex
    /// holding nothing at all. Neither has a boundary to draw, and guarding
    /// explicitly is better than letting marching squares decide: with all
    /// weights the same sign `F` never truly crosses zero, but far-out cells
    /// underflow to exactly `0.0`, and a zero corner is ambiguous.
    pub fn sample(sources: &[Source], bounds: Bounds, spacing: f32, model: Model) -> Option<Field> {
        let has_colonial = sources.iter().any(|source| source.weight > 0.0);
        let has_warden = sources.iter().any(|source| source.weight < 0.0);

        if !has_colonial || !has_warden {
            return None;
        }

        let spacing = spacing.max(1.0);
        let cols = ((bounds.width() / spacing).ceil() as usize)
            .saturating_add(1)
            .max(2);
        let rows = ((bounds.height() / spacing).ceil() as usize)
            .saturating_add(1)
            .max(2);

        let mut values = Vec::with_capacity(cols * rows);

        for row in 0..rows {
            let y = bounds.min_y + row as f32 * spacing;

            for col in 0..cols {
                let x = bounds.min_x + col as f32 * spacing;
                values.push(influence(sources, x, y, model));
            }
        }

        Some(Field {
            origin: (bounds.min_x, bounds.min_y),
            spacing,
            cols,
            rows,
            values,
        })
    }

    fn value(&self, col: usize, row: usize) -> f32 {
        self.values[row * self.cols + col]
    }

    /// Grid coordinate to world pixel.
    fn world(&self, col: f32, row: f32) -> Point {
        (
            self.origin.0 + col * self.spacing,
            self.origin.1 + row * self.spacing,
        )
    }
}

/// `F` at one point.
///
/// # What `k` does, and why it is 1
///
/// Put one source at distance `a` against `n` sources at distance `b` and solve
/// `1/a^(2k) = n/b^(2k)`: the balance sits at `b/a = n^(1/(2k))`. So `k` bounds
/// how much a crowd is worth, and the *sign* of `Σ w/(d²+ε)^k` is the sign of
/// the difference between the two sides' `p`-norm soft-minimum distances at
/// `p = 2k` — raising `k` walks the contour toward the true medial axis between
/// the two point sets, and in the limit it *is* the Voronoi partition the spec
/// rejects.
///
/// **This was tried as the fix for a line that leaned toward whoever had built
/// more, and it was the wrong lever.** The reasoning was right — at `k = 1` a
/// huddle of nine held ground three times as far out as a lone base, and forty
/// held it six times — but the conclusion was not, because it treated `n` as
/// given. Against the second round of live-war feedback, on a fixture carrying
/// the same 3:1 density asymmetry as the real map:
///
/// | | off centre | total turning |
/// |---|---|---|
/// | every icon, `k = 1` | +138 px | 158° |
/// | every icon, `k = 2` | +68 px | 90° |
/// | **one point per footing, `k = 1`** | **+33 px** | **42°** |
/// | one point per footing, `k = 2` | +21 px | 60° |
///
/// [`footings`] beats the exponent at its own job and *improves* smoothness
/// where raising `k` cost it — which is the angular line the second round
/// complained about. And once the point set is footings rather than buildings,
/// a count difference means one side genuinely holds more ground there, so
/// `k > 1` would be suppressing signal rather than noise.
///
/// It stays a knob because it is a real dial with a measured effect, and
/// because the argument above only holds while the point set stays clustered.
///
/// Accumulated in `f64` because the field sums terms spanning a dozen orders of
/// magnitude and `f32` loses the tail of that sum — which is exactly the
/// far-field contribution that decides where a boundary sits in the quiet
/// stretches between clusters.
fn influence(sources: &[Source], x: f32, y: f32, model: Model) -> f32 {
    let epsilon = f64::from(model.epsilon);
    // Floored at 1: a zero exponent collapses `F` to the difference of the two
    // sides' structure counts, which is constant across the map and has no zero
    // contour at all. Capped at 8 because past there the far field underflows
    // and the model is Voronoi in all but name.
    let falloff = model.falloff.clamp(1, 8) as i32;

    let total: f64 = sources
        .iter()
        .map(|source| {
            let dx = f64::from(x - source.x);
            let dy = f64::from(y - source.y);

            f64::from(source.weight) / (dx * dx + dy * dy + epsilon).powi(falloff)
        })
        .sum();

    total as f32
}

/// Rounds of Chaikin the renderer uses. Two is enough to take the staircase out
/// without pulling the line noticeably away from where the field put it.
pub const SMOOTHING_ROUNDS: usize = 2;

/// Polylines below this many points are dropped as noise: a contour enclosing
/// less than a cell says more about the sampling grid than about the war.
const MIN_POLYLINE_POINTS: usize = 3;

/// Marching squares on `F = 0`, joined into polylines in world space.
///
/// Run directly on the sampled grid, and the *polyline* is smoothed afterwards
/// rather than the field being interpolated up first. Both reach the same
/// place — bilinear interpolation cannot invent a crossing inside a cell whose
/// four corners agree in sign, so it changes nothing about the island
/// behaviour below — and this way there is one grid rather than two.
///
/// This is also where the "lone outpost makes no island" criterion is actually
/// met, and it is worth being straight about the mechanism. The *field* does not
/// quite manage it on its own: within about `sqrt(ε)` of an isolated enemy
/// structure its own `w / ε` term does outweigh the surrounding side, so `F`
/// genuinely does cross zero there. What kills the island is that the crossing
/// is a few tens of pixels across and the grid is sampled hundreds of pixels
/// apart, so it falls entirely between samples. The coarse grid is a low-pass
/// filter on the field, not just a cost saving — which is the honest reason not
/// to "improve" the feature by sampling it finely.
pub fn contour(field: &Field) -> Vec<Polyline> {
    let mut segments = Vec::new();

    for row in 0..field.rows.saturating_sub(1) {
        for col in 0..field.cols.saturating_sub(1) {
            let top_left = field.value(col, row);
            let top_right = field.value(col + 1, row);
            let bottom_right = field.value(col + 1, row + 1);
            let bottom_left = field.value(col, row + 1);

            let case = usize::from(top_left > 0.0) << 3
                | usize::from(top_right > 0.0) << 2
                | usize::from(bottom_right > 0.0) << 1
                | usize::from(bottom_left > 0.0);

            // Every edge is interpolated from the same two corners in the same
            // order by both cells that share it, so the endpoints of adjoining
            // segments come out bit-identical and the chaining below joins them
            // without a tolerance.
            let top = || field.world(col as f32 + crossing(top_left, top_right), row as f32);
            let bottom = || {
                field.world(
                    col as f32 + crossing(bottom_left, bottom_right),
                    row as f32 + 1.0,
                )
            };
            let left = || field.world(col as f32, row as f32 + crossing(top_left, bottom_left));
            let right = || {
                field.world(
                    col as f32 + 1.0,
                    row as f32 + crossing(top_right, bottom_right),
                )
            };

            match case {
                0 | 15 => {}
                1 | 14 => segments.push((left(), bottom())),
                2 | 13 => segments.push((bottom(), right())),
                3 | 12 => segments.push((left(), right())),
                4 | 11 => segments.push((top(), right())),
                6 | 9 => segments.push((top(), bottom())),
                7 | 8 => segments.push((left(), top())),
                // The saddles. Which way the contour turns is decided by the
                // cell's own centre: if it sits with the positive corners, they
                // are connected through the middle and it is the negative pair
                // that gets cut off.
                5 | 10 => {
                    let centre = (top_left + top_right + bottom_right + bottom_left) / 4.0;
                    let positive_centre = centre > 0.0;
                    let corners_are_positive = case == 10;

                    if positive_centre == corners_are_positive {
                        segments.push((left(), top()));
                        segments.push((bottom(), right()));
                    } else {
                        segments.push((left(), bottom()));
                        segments.push((top(), right()));
                    }
                }
                _ => unreachable!("a four-bit case is 0..=15"),
            }
        }
    }

    chain(segments)
        .into_iter()
        .filter(|line| line.len() >= MIN_POLYLINE_POINTS)
        .collect()
}

/// Where the zero crossing sits between two corner values, as a fraction of the
/// edge. Both are known to straddle zero, so the denominator cannot vanish
/// except on two exact zeros, which the clamp catches.
fn crossing(from: f32, to: f32) -> f32 {
    let span = from - to;

    if span.abs() < f32::EPSILON {
        0.5
    } else {
        (from / span).clamp(0.0, 1.0)
    }
}

/// Sixteenths of a pixel: fine enough that two genuinely different crossings
/// never collide, coarse enough to absorb a last-bit difference if one ever
/// slips through.
fn key(point: Point) -> (i32, i32) {
    (
        (point.0 * 16.0).round() as i32,
        (point.1 * 16.0).round() as i32,
    )
}

/// Joins loose segments end to end into as few polylines as possible.
fn chain(segments: Vec<Segment>) -> Vec<Polyline> {
    use std::collections::HashMap;

    let mut ends: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

    for (index, (start, end)) in segments.iter().enumerate() {
        ends.entry(key(*start)).or_default().push(index);
        ends.entry(key(*end)).or_default().push(index);
    }

    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();

    for start_index in 0..segments.len() {
        if used[start_index] {
            continue;
        }

        used[start_index] = true;

        let (start, end) = segments[start_index];
        let mut line = vec![start, end];

        // Forward from the tail, then backward from the head. A closed loop
        // finishes when the forward walk arrives back at the head, and is left
        // with its first point repeated at the end so the smoothing below can
        // tell it is closed.
        extend(&mut line, &segments, &ends, &mut used);
        line.reverse();
        extend(&mut line, &segments, &ends, &mut used);

        lines.push(line);
    }

    lines
}

fn extend(
    line: &mut Polyline,
    segments: &[Segment],
    ends: &std::collections::HashMap<(i32, i32), Vec<usize>>,
    used: &mut [bool],
) {
    loop {
        let tail = key(*line.last().expect("a chain always starts with two points"));

        let Some(next) = ends
            .get(&tail)
            .into_iter()
            .flatten()
            .copied()
            .find(|index| !used[*index])
        else {
            return;
        };

        used[next] = true;

        let (start, end) = segments[next];
        line.push(if key(start) == tail { end } else { start });
    }
}

/// Chaikin corner cutting, which is what turns marching squares' staircase into
/// the gentle curve the reference has (slope changing by at most 1 px per px).
///
/// Endpoints of an open line are kept: a frontline is supposed to reach the hex
/// edge, and pulling its ends inward is the gap the reference has and the spec
/// calls a defect.
pub fn smooth(lines: Vec<Polyline>, rounds: usize) -> Vec<Polyline> {
    lines
        .into_iter()
        .map(|line| (0..rounds).fold(line, |line, _| chaikin(line)))
        .collect()
}

fn chaikin(line: Polyline) -> Polyline {
    if line.len() < 3 {
        return line;
    }

    let closed = key(line[0]) == key(line[line.len() - 1]);
    let mut cut = Vec::with_capacity(line.len() * 2);

    if !closed {
        cut.push(line[0]);
    }

    for pair in line.windows(2) {
        let (from, to) = (pair[0], pair[1]);

        cut.push((from.0 * 0.75 + to.0 * 0.25, from.1 * 0.75 + to.1 * 0.25));
        cut.push((from.0 * 0.25 + to.0 * 0.75, from.1 * 0.25 + to.1 * 0.75));
    }

    if closed {
        // Keep it closed: the first cut point is what the last segment should
        // run back to.
        let first = cut[0];
        cut.push(first);
    } else {
        cut.push(line[line.len() - 1]);
    }

    cut
}

/// A traced boundary, with the faction whose ground lies on each side of it.
///
/// The sides are carried alongside the points because they **cannot be
/// recovered from them**. [`chain`] walks segments in whatever order it finds
/// them and reverses them freely to attach, so a polyline's direction is an
/// accident of iteration order rather than anything about the war. Inferring
/// the flank from the winding would be right about half the time — and per
/// polyline, so it would look correct in one screenshot and inverted in the
/// next, which is the worst way for this to be wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub points: Polyline,
    /// One entry per segment, so `points.len() - 1` long. `true` when Colonial
    /// ground lies on the `(-dy, dx)` side of the walk from `points[i]` to
    /// `points[i + 1]` — the side a renderer finds by the sign of
    /// `dx * (py - y0) - dy * (px - x0)` being positive.
    pub colonial_side: Vec<bool>,
}

/// Asks the field which flank is whose, one segment at a time.
///
/// Sampled on both sides and compared, rather than testing the sign of a single
/// probe. The line sits where `F = 0` and Chaikin then moves it by up to a cell,
/// so a lone sample can land back across the contour and read the wrong side;
/// the *difference* between the two only flips if the probe overshoots the far
/// side of the front entirely.
///
/// `probe` is that step, in world pixels. One field cell is the right size: the
/// contour is located to about a cell, so anything shorter is inside the noise
/// and anything much longer starts sampling a different part of the front where
/// the line curves.
///
/// Per segment rather than per polyline, though the answer is constant along a
/// connected walk: it costs a few thousand field evaluations against 21 ms of
/// sampling, and it cannot be broken by a contour that pinches or by segments
/// that arrive out of order.
pub fn flanks(lines: Vec<Polyline>, sources: &[Source], model: Model, probe: f32) -> Vec<Edge> {
    lines
        .into_iter()
        .map(|points| {
            let mut colonial_side = Vec::with_capacity(points.len().saturating_sub(1));

            for pair in points.windows(2) {
                let ((from_x, from_y), (to_x, to_y)) = (pair[0], pair[1]);
                let (dx, dy) = (to_x - from_x, to_y - from_y);
                let length = (dx * dx + dy * dy).sqrt();

                if length <= f32::EPSILON {
                    // Marching squares can emit a crossing twice on a grid
                    // corner. It has no sides; inherit the run it sits in so
                    // the flags stay aligned with the segments.
                    colonial_side.push(colonial_side.last().copied().unwrap_or(true));
                    continue;
                }

                let (step_x, step_y) = (-dy / length * probe, dx / length * probe);
                let (mid_x, mid_y) = ((from_x + to_x) / 2.0, (from_y + to_y) / 2.0);

                colonial_side.push(
                    influence(sources, mid_x + step_x, mid_y + step_y, model)
                        > influence(sources, mid_x - step_x, mid_y - step_y, model),
                );
            }

            Edge {
                points,
                colonial_side,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hex-sized window, which is the geometry every case here is about.
    fn window() -> Bounds {
        Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: REGION_WIDTH as f32,
            max_y: REGION_HEIGHT as f32,
        }
    }

    fn colonial(x: f32, y: f32) -> Source {
        Source { x, y, weight: 1.0 }
    }

    fn warden(x: f32, y: f32) -> Source {
        Source { x, y, weight: -1.0 }
    }

    /// Two clusters, one either side of the hex's vertical midline.
    fn contested() -> Vec<Source> {
        let mut sources = Vec::new();

        for step in 0..5 {
            let y = 150.0 + step as f32 * 150.0;
            sources.push(colonial(200.0, y));
            sources.push(warden(824.0, y));
        }

        sources
    }

    /// A front that actually bends, which the symmetric fixture above does not:
    /// two ranks converging across the hex, so marching squares has corners to
    /// cut.
    const SPACING: f32 = 16.0;

    /// What the renderer actually uses, so these exercise the shipped field
    /// rather than a shape nothing renders with.
    fn model() -> Model {
        RenderConfig::default().influence_model()
    }

    #[test]
    fn one_faction_alone_has_no_field() {
        let held = vec![colonial(200.0, 400.0), colonial(300.0, 500.0)];

        assert!(Field::sample(&held, window(), SPACING, model()).is_none());
    }

    #[test]
    fn an_empty_hex_has_no_field() {
        assert!(Field::sample(&[], window(), SPACING, model()).is_none());
    }

    #[test]
    fn neutral_structures_count_for_neither_side() {
        let region =
            crate::utils::regions::find("DeadLandsHex").expect("Deadlands is in the table");
        let config = RenderConfig::default();

        let dynamic = DynamicMapData {
            region_id: 0,
            scorched_victory_towns: 0,
            last_updated: 0,
            version: 1,
            map_items: vec![
                item(TeamId::Colonials, 0.25, 0.5),
                item(TeamId::None, 0.5, 0.5),
                item(TeamId::Wardens, 0.75, 0.5),
            ],
        };

        let sources = sources_in(region, &dynamic, &config);

        assert_eq!(sources.len(), 2);
        assert!(sources.iter().all(|source| source.weight != 0.0));
    }

    fn item(team_id: TeamId, x: f64, y: f64) -> crate::utils::api_definitions::foxhole::MapItem {
        crate::utils::api_definitions::foxhole::MapItem {
            team_id,
            icon_type: 5,
            x,
            y,
            flags: 0,
            view_direction: 0,
        }
    }

    #[test]
    fn structures_land_where_the_grid_puts_their_region() {
        let region =
            crate::utils::regions::find("DeadLandsHex").expect("Deadlands is in the table");
        let config = RenderConfig::default();
        let (origin_x, origin_y) = config.grid_offset(region.col, region.row);

        let dynamic = DynamicMapData {
            region_id: 0,
            scorched_victory_towns: 0,
            last_updated: 0,
            version: 1,
            map_items: vec![item(TeamId::Colonials, 0.5, 0.5)],
        };

        let placed = sources_in(region, &dynamic, &config);

        assert_eq!(placed.len(), 1);
        assert!((placed[0].x - (origin_x as f32 + REGION_WIDTH as f32 / 2.0)).abs() < 0.5);
        assert!((placed[0].y - (origin_y as f32 + REGION_HEIGHT as f32 / 2.0)).abs() < 0.5);
    }

    #[test]
    fn the_field_is_signed_toward_whoever_is_nearer() {
        // Straight at `influence`, which is `F` itself — no grid needed to ask
        // which way it leans.
        let sources = contested();

        assert!(
            influence(&sources, 200.0, 450.0, model()) > 0.0,
            "colonial side"
        );
        assert!(
            influence(&sources, 824.0, 450.0, model()) < 0.0,
            "warden side"
        );
        assert!(
            influence(&sources, 512.0, 450.0, model()).abs()
                < influence(&sources, 200.0, 450.0, model()),
            "and is weakest between them"
        );
    }

    #[test]
    fn two_clusters_produce_one_line_between_them() {
        let field = Field::sample(&contested(), window(), SPACING, model()).expect("contested");
        let lines = contour(&field);

        assert_eq!(lines.len(), 1, "one front, not several: {lines:?}");

        for &(x, _) in &lines[0] {
            assert!(
                (200.0..=824.0).contains(&x),
                "the line left the ground between the clusters at x={x}"
            );
        }
    }

    #[test]
    fn the_line_runs_from_one_edge_to_the_other() {
        let field = Field::sample(&contested(), window(), SPACING, model()).expect("contested");
        let lines = contour(&field);
        let line = &lines[0];

        let top = line.iter().map(|point| point.1).fold(f32::MAX, f32::min);
        let bottom = line.iter().map(|point| point.1).fold(f32::MIN, f32::max);

        assert!(top <= SPACING, "starts at the top edge, got {top}");
        assert!(
            bottom >= REGION_HEIGHT as f32 - SPACING,
            "runs to the bottom edge, got {bottom}"
        );
    }

    /// The criterion the Voronoi model was rejected for. See [`contour`] for
    /// why the coarse grid rather than the field is what delivers it.
    #[test]
    fn a_lone_outpost_makes_no_island() {
        let mut sources = contested();
        sources.push(warden(200.0, 450.0));

        let field = Field::sample(&sources, window(), 64.0, model()).expect("contested");
        let lines = contour(&field);

        for line in &lines {
            assert!(
                key(line[0]) != key(line[line.len() - 1]),
                "a closed loop appeared around the lone outpost: {line:?}"
            );
        }
    }

    /// The live-war complaint, reduced to the smallest case that shows it: a
    /// lone Colonial base against a built-up Warden town, the same distance
    /// either side of the middle.
    ///
    /// The boundary belongs at x = 512, and what put it at 350 was not distance
    /// but headcount — nine icons against one. Clustering is the fix; the two
    /// assertions are the two things it has to get right, and the second is the
    /// one a naive "just merge nearby points" would fail.
    #[test]
    fn a_crowd_does_not_buy_ground() {
        let mut sources = vec![colonial(212.0, 444.0)];

        for step in 0..9 {
            sources.push(warden(
                812.0 + (step % 3) as f32 * 20.0,
                404.0 + (step / 3) as f32 * 40.0,
            ));
        }

        // Walk the midline for the sign change: the question with none of the
        // grid or the chaining in the way.
        let crossing = |sources: &[Source]| {
            (212..=812)
                .find(|x| influence(sources, *x as f32, 444.0, model()) < 0.0)
                .expect("the field has to flip somewhere between them") as f32
        };

        let config = RenderConfig::default();
        let raw = (crossing(&sources) - 512.0).abs();
        let clustered = (crossing(&footings(&sources, config.footing_radius())) - 512.0).abs();

        assert_eq!(
            footings(&sources, config.footing_radius()).len(),
            2,
            "one town and one base is two footings"
        );
        assert!(
            clustered < 20.0,
            "one base against one town is a symmetric problem and belongs in the \
             middle: counting every icon put it {raw} px off centre, clustering \
             leaves {clustered}"
        );
    }

    /// The failure a grid-bucketed clustering would have, and the reason this
    /// one is greedy: the two sides of a contested river are within a footing
    /// radius of each other, and merging them would erase the boundary.
    #[test]
    fn opposing_footings_never_merge() {
        let sources = vec![
            colonial(500.0, 444.0),
            colonial(510.0, 450.0),
            warden(530.0, 444.0),
            warden(540.0, 450.0),
        ];

        let merged = footings(&sources, 200.0);

        assert_eq!(merged.len(), 2, "one footing per side, not one in total");
        assert!(merged.iter().any(|source| source.weight > 0.0));
        assert!(merged.iter().any(|source| source.weight < 0.0));
    }

    #[test]
    fn a_footing_sits_at_the_centre_of_what_it_absorbed() {
        let sources = vec![
            colonial(400.0, 400.0),
            colonial(440.0, 400.0),
            colonial(400.0, 440.0),
            colonial(440.0, 440.0),
            // Far enough out to stay its own footing.
            colonial(900.0, 400.0),
        ];

        let merged = footings(&sources, 100.0);

        assert_eq!(merged.len(), 2);
        assert!((merged[0].x - 420.0).abs() < 0.01, "got {}", merged[0].x);
        assert!((merged[0].y - 420.0).abs() < 0.01, "got {}", merged[0].y);
        assert_eq!(
            merged[0].weight, 1.0,
            "a town of four weighs the same as the lone base — one foothold"
        );
    }

    #[test]
    fn the_flank_follows_the_field_and_not_the_winding() {
        // Colonials west, Wardens east, so the line runs north-south and
        // Colonial ground is on whichever side faces decreasing x.
        let sources = vec![colonial(120.0, 250.0), warden(380.0, 250.0)];

        let walks = [
            vec![(250.0, 100.0), (250.0, 400.0)],
            vec![(250.0, 400.0), (250.0, 100.0)],
        ];

        for points in walks {
            let downward = points[1].1 > points[0].1;
            let edges = flanks(vec![points], &sources, model(), 32.0);

            // Walking down the map, `(-dy, dx)` points west; walking up, east.
            // So a flag that reports the field faithfully has to flip with the
            // direction, and one derived from the winding would not.
            assert_eq!(
                edges[0].colonial_side,
                vec![downward],
                "the Colonial flank moved when only the walk direction did"
            );
        }
    }

    #[test]
    fn every_segment_gets_a_side() {
        let edges = flanks(
            vec![vec![(250.0, 100.0), (250.0, 250.0), (250.0, 400.0)]],
            &contested(),
            model(),
            32.0,
        );

        // The renderer indexes the flags by segment. One short and the last
        // segment silently falls back to a default.
        assert_eq!(edges[0].colonial_side.len(), edges[0].points.len() - 1);
    }

    fn contested_wavy() -> Vec<Source> {
        let mut sources = Vec::new();

        for step in 0..5 {
            let t = step as f32 / 4.0;
            sources.push(colonial(120.0 + t * 780.0, 150.0 + t * 180.0));
            sources.push(warden(180.0 + t * 780.0, 740.0 - t * 180.0));
        }

        sources
    }

    #[test]
    fn smoothing_takes_the_staircase_out() {
        let field = Field::sample(&contested_wavy(), window(), 64.0, model()).expect("contested");
        let raw = contour(&field);
        let smoothed = smooth(raw.clone(), 2);

        assert_eq!(smoothed.len(), raw.len());

        // The two halves of what corner cutting is for. The sharpest single
        // direction change falls (0.18 rad to 0.11 on this fixture) while the
        // total turning is untouched: Chaikin spreads the same curve over more,
        // smaller steps rather than straightening the line, which is why it
        // smooths the staircase without moving where the front sits.
        assert!(worst_turn(&smoothed[0]) < worst_turn(&raw[0]));
        assert!((total_turn(&smoothed[0]) - total_turn(&raw[0])).abs() < 0.01);
    }

    #[test]
    fn smoothing_keeps_the_ends_where_they_were() {
        let field = Field::sample(&contested_wavy(), window(), 64.0, model()).expect("contested");
        let raw = contour(&field);
        let smoothed = smooth(raw.clone(), 2);

        assert_eq!(smoothed[0][0], raw[0][0]);
        assert_eq!(
            smoothed[0][smoothed[0].len() - 1],
            raw[0][raw[0].len() - 1],
            "the ends have to stay on the edge, or the line stops short of the silhouette"
        );
    }

    /// Direction change from one segment to the next, wrapped into +/- pi.
    fn turns(line: &[Point]) -> impl Iterator<Item = f32> + '_ {
        line.windows(3).map(|window| {
            let first = (window[1].1 - window[0].1).atan2(window[1].0 - window[0].0);
            let second = (window[2].1 - window[1].1).atan2(window[2].0 - window[1].0);

            let mut turn = second - first;

            while turn > std::f32::consts::PI {
                turn -= 2.0 * std::f32::consts::PI;
            }
            while turn < -std::f32::consts::PI {
                turn += 2.0 * std::f32::consts::PI;
            }

            turn.abs()
        })
    }

    fn worst_turn(line: &[Point]) -> f32 {
        turns(line).fold(0.0, f32::max)
    }

    fn total_turn(line: &[Point]) -> f32 {
        turns(line).sum()
    }
}


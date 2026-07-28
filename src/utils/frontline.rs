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
    /// negative Warden. Magnitude is the `w` in the model — every structure
    /// weighs 1 today, and per-type weights are the first thing to try if the
    /// line sits wrong against a live war.
    pub weight: f32,
}

/// Every faction-held structure in one region, placed in world space.
///
/// **Every** structure, not just `CONTROL_ICON_TYPES`. The tint's reason for
/// narrowing to bases does not transfer: it *counts* and takes a majority, so
/// sheds inflate a total, where a field *sums by distance* and a structure deep
/// in friendly ground contributes nothing to where the boundary sits. Neutral
/// structures belong to neither side and are dropped.
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
    /// Evaluates `F(p) = Σ w / (d² + ε)` over `bounds`.
    ///
    /// `None` when one side has nothing here — which covers both degenerate
    /// cases the spec names, a hex held entirely by one faction and a hex
    /// holding nothing at all. Neither has a boundary to draw, and guarding
    /// explicitly is better than letting marching squares decide: with all
    /// weights the same sign `F` never truly crosses zero, but far-out cells
    /// underflow to exactly `0.0`, and a zero corner is ambiguous.
    pub fn sample(sources: &[Source], bounds: Bounds, spacing: f32, epsilon: f32) -> Option<Field> {
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
                values.push(influence(sources, x, y, epsilon));
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

    /// The field at an arbitrary world point, bilinearly interpolated.
    ///
    /// Interpolating up beats sampling finely: the cost of `sample` is
    /// O(cells x sources), and this is O(1).
    pub fn value_at(&self, x: f32, y: f32) -> f32 {
        let grid_x = ((x - self.origin.0) / self.spacing).clamp(0.0, (self.cols - 1) as f32);
        let grid_y = ((y - self.origin.1) / self.spacing).clamp(0.0, (self.rows - 1) as f32);

        let col = grid_x.floor() as usize;
        let row = grid_y.floor() as usize;
        let next_col = (col + 1).min(self.cols - 1);
        let next_row = (row + 1).min(self.rows - 1);

        let tx = grid_x - col as f32;
        let ty = grid_y - row as f32;

        let top = self.value(col, row) * (1.0 - tx) + self.value(next_col, row) * tx;
        let bottom = self.value(col, next_row) * (1.0 - tx) + self.value(next_col, next_row) * tx;

        top * (1.0 - ty) + bottom * ty
    }

    /// Grid coordinate to world pixel.
    fn world(&self, col: f32, row: f32) -> Point {
        (
            self.origin.0 + col * self.spacing,
            self.origin.1 + row * self.spacing,
        )
    }
}

/// `F` at one point. Accumulated in `f64` because a full-map field sums a couple
/// of thousand terms that are each around 1e-6, and `f32` loses the tail of that
/// sum — which is exactly the far-field contribution that decides where a
/// boundary sits in the quiet stretches between clusters.
fn influence(sources: &[Source], x: f32, y: f32, epsilon: f32) -> f32 {
    let total: f64 = sources
        .iter()
        .map(|source| {
            let dx = f64::from(x - source.x);
            let dy = f64::from(y - source.y);

            f64::from(source.weight) / (dx * dx + dy * dy + f64::from(epsilon))
        })
        .sum();

    total as f32
}

/// Polylines below this many points are dropped as noise: a contour enclosing
/// less than a cell says more about the sampling grid than about the war.
const MIN_POLYLINE_POINTS: usize = 3;

/// Marching squares on `F = 0`, joined into polylines in world space.
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
    const EPSILON: f32 = 2500.0;
    const SPACING: f32 = 16.0;

    #[test]
    fn one_faction_alone_has_no_field() {
        let held = vec![colonial(200.0, 400.0), colonial(300.0, 500.0)];

        assert!(Field::sample(&held, window(), SPACING, EPSILON).is_none());
    }

    #[test]
    fn an_empty_hex_has_no_field() {
        assert!(Field::sample(&[], window(), SPACING, EPSILON).is_none());
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
        let field = Field::sample(&contested(), window(), SPACING, EPSILON).expect("contested");

        assert!(field.value_at(200.0, 450.0) > 0.0, "colonial side");
        assert!(field.value_at(824.0, 450.0) < 0.0, "warden side");
    }

    #[test]
    fn two_clusters_produce_one_line_between_them() {
        let field = Field::sample(&contested(), window(), SPACING, EPSILON).expect("contested");
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
        let field = Field::sample(&contested(), window(), SPACING, EPSILON).expect("contested");
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

        let field = Field::sample(&sources, window(), 64.0, EPSILON).expect("contested");
        let lines = contour(&field);

        for line in &lines {
            assert!(
                key(line[0]) != key(line[line.len() - 1]),
                "a closed loop appeared around the lone outpost: {line:?}"
            );
        }
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
        let field = Field::sample(&contested_wavy(), window(), 64.0, EPSILON).expect("contested");
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
        let field = Field::sample(&contested_wavy(), window(), 64.0, EPSILON).expect("contested");
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

    #[test]
    fn interpolation_agrees_with_the_samples_it_sits_between() {
        let field = Field::sample(&contested(), window(), SPACING, EPSILON).expect("contested");

        // On a sample, exactly; between two, between their values.
        assert_eq!(field.value_at(0.0, 0.0), field.value(0, 0));

        let midpoint = field.value_at(SPACING / 2.0, 0.0);
        let (low, high) = (field.value(0, 0), field.value(1, 0));

        assert!(midpoint >= low.min(high) && midpoint <= low.max(high));
    }
}

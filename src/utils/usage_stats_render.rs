//! PNG dashboard for the reviewer-only `/usage-stats` command.

use std::collections::HashMap;
use std::io::Cursor;

use ab_glyph::{FontRef, PxScale};
use image::{ImageBuffer, ImageFormat, Rgba};
use imageproc::drawing::draw_text_mut;
use thiserror::Error;

use crate::utils::db::{UsageBucket, UsageOverview};
use crate::utils::usage::{
    canonical_name, SCHEDULED_FULL_MAP, SCHEDULED_REPORT, SCHEDULE_COMMANDS,
};

const WIDTH: u32 = 1600;
const PAD: i32 = 48;
const FONT: &[u8] = include_bytes!("../../assets/Inter-Bold.ttf");
const BG: Rgba<u8> = Rgba([18, 24, 29, 255]);
const PANEL: Rgba<u8> = Rgba([29, 39, 46, 255]);
const PANEL_ALT: Rgba<u8> = Rgba([35, 47, 55, 255]);
const TEXT: Rgba<u8> = Rgba([232, 237, 239, 255]);
const MUTED: Rgba<u8> = Rgba([153, 169, 176, 255]);
const BLUE: Rgba<u8> = Rgba([92, 166, 219, 255]);
const GOLD: Rgba<u8> = Rgba([211, 173, 79, 255]);

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("the bundled font could not be parsed")]
    Font,
    #[error("could not encode usage statistics as PNG: {0}")]
    Encode(#[from] image::ImageError),
}

pub fn render(
    overview: &UsageOverview,
    daily: &[UsageBucket],
    daily_buckets: &[String],
    weekly: &[UsageBucket],
    weekly_buckets: &[String],
) -> Result<Vec<u8>, RenderError> {
    let font = FontRef::try_from_slice(FONT).map_err(|_| RenderError::Font)?;
    let row_height = 34;
    let table_height = 70 + row_height * daily_buckets.len().max(weekly_buckets.len()) as i32;
    let height = 270 + table_height + 70;
    let mut canvas = ImageBuffer::from_pixel(WIDTH, height as u32, BG);

    text(&mut canvas, &font, "USAGE REPORT", PAD, 38, 34.0, TEXT);
    text(
        &mut canvas,
        &font,
        "GLOBAL OPERATIONS // UTC",
        PAD,
        82,
        16.0,
        MUTED,
    );

    let cards = [
        ("CONFIGURED SERVERS", overview.guilds),
        ("APPROVED SERVERS", overview.approved_guilds),
        ("LIVE SCHEDULES", overview.schedules),
        ("FULL-MAP SCHEDULES", overview.full_map_schedules),
        ("SCHEDULING SERVERS", overview.scheduling_guilds),
    ];
    let card_gap = 16;
    let card_width = (WIDTH as i32 - PAD * 2 - card_gap * 4) / 5;
    for (index, (label, value)) in cards.into_iter().enumerate() {
        let x = PAD + index as i32 * (card_width + card_gap);
        panel(&mut canvas, x, 125, card_width, 105, PANEL);
        text(&mut canvas, &font, label, x + 18, 143, 13.0, MUTED);
        text(
            &mut canvas,
            &font,
            &value.to_string(),
            x + 18,
            169,
            31.0,
            BLUE,
        );
    }

    let table_y = 270;
    let table_gap = 24;
    let table_width = (WIDTH as i32 - PAD * 2 - table_gap) / 2;
    table(
        &mut canvas,
        &font,
        "DAILY ACTIVITY",
        "DAY",
        daily,
        daily_buckets,
        PAD,
        table_y,
        table_width,
        table_height,
    );
    table(
        &mut canvas,
        &font,
        "WEEKLY ACTIVITY",
        "MONDAY",
        weekly,
        weekly_buckets,
        PAD + table_width + table_gap,
        table_y,
        table_width,
        table_height,
    );

    text(
        &mut canvas,
        &font,
        "COUNTS ARE PER SERVER PER UTC DAY. NO USER IDENTIFIERS ARE STORED.",
        PAD,
        height - 42,
        14.0,
        MUTED,
    );

    let mut output = Cursor::new(Vec::new());
    canvas.write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}

#[allow(clippy::too_many_arguments)]
fn table(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    font: &FontRef<'_>,
    title: &str,
    bucket_label: &str,
    rows: &[UsageBucket],
    buckets: &[String],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    panel(canvas, x, y, width, height, PANEL);
    text(canvas, font, title, x + 20, y + 18, 18.0, GOLD);
    text(canvas, font, bucket_label, x + 20, y + 52, 12.0, MUTED);
    for (index, column) in [
        "GET-MAP", "FULL-MAP", "REPORTS", "SCH-CMD", "OTHER", "SERVERS",
    ]
    .into_iter()
    .enumerate()
    {
        text(
            canvas,
            font,
            column,
            x + 175 + index as i32 * 83,
            y + 52,
            11.0,
            MUTED,
        );
    }

    let mut counts = HashMap::new();
    for row in rows {
        let entry = counts
            .entry(row.bucket.as_str())
            .or_insert(([0i64; 5], 0i64));
        if let Some(command) = &row.command {
            entry.0[column_of(command)] += row.uses;
        } else {
            entry.1 = row.servers;
        }
    }

    for (index, bucket) in buckets.iter().enumerate() {
        let row_y = y + 72 + index as i32 * 34;
        if index % 2 == 0 {
            panel(canvas, x + 12, row_y - 5, width - 24, 32, PANEL_ALT);
        }
        let (values, servers) = counts.get(bucket.as_str()).copied().unwrap_or_default();
        let label = bucket.get(..10).unwrap_or(bucket);
        text(canvas, font, label, x + 20, row_y, 13.0, TEXT);
        for (column, value) in values.into_iter().enumerate() {
            text(
                canvas,
                font,
                &value.to_string(),
                x + 190 + column as i32 * 83,
                row_y,
                13.0,
                TEXT,
            );
        }
        text(
            canvas,
            font,
            &servers.to_string(),
            x + 605,
            row_y,
            13.0,
            TEXT,
        );
    }
}

fn column_of(command: &str) -> usize {
    let normalized = canonical_name(command);
    match normalized.as_str() {
        "get-map" => 0,
        "full-map" => 1,
        SCHEDULED_REPORT | SCHEDULED_FULL_MAP => 2,
        _ if SCHEDULE_COMMANDS.contains(&normalized.as_str()) => 3,
        _ => 4,
    }
}

fn panel(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: Rgba<u8>,
) {
    for py in y.max(0)..(y + height).min(canvas.height() as i32) {
        for px in x.max(0)..(x + width).min(canvas.width() as i32) {
            canvas.put_pixel(px as u32, py as u32, color);
        }
    }
}

fn text(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    font: &FontRef<'_>,
    value: &str,
    x: i32,
    y: i32,
    size: f32,
    color: Rgba<u8>,
) {
    draw_text_mut(canvas, color, x, y, PxScale::from(size), font, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_png_with_all_requested_buckets() {
        let overview = UsageOverview {
            guilds: 3,
            approved_guilds: 2,
            schedules: 4,
            full_map_schedules: 1,
            scheduling_guilds: 2,
        };
        let daily = vec!["2026-08-10".to_string(); 14];
        let weekly = vec!["2026-08-10".to_string(); 8];
        let png = render(&overview, &[], &daily, &[], &weekly).unwrap();
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!(image.width(), WIDTH);
        assert!(image.height() > 800);
    }

    #[test]
    fn legacy_snake_case_names_use_their_dashboard_columns() {
        assert_eq!(column_of("get_map"), 0);
        assert_eq!(column_of("full_map"), 1);
        assert_eq!(column_of("schedule_report"), 3);
    }
}

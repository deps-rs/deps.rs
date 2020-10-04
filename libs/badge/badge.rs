//! Simple badge generator

use base64::display::Base64Display;
use once_cell::sync::Lazy;
use rusttype::{point, Font, Point, PositionedGlyph, Scale};

const FONT_DATA: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/DejaVuSans.ttf"));
const FONT_SIZE: f32 = 11.;
const SCALE: Scale = Scale {
    x: FONT_SIZE,
    y: FONT_SIZE,
};

pub struct BadgeOptions {
    /// Subject will be displayed on the left side of badge
    pub subject: String,
    /// Status will be displayed on the right side of badge
    pub status: String,
    /// HTML color of badge
    pub color: String,
}

impl Default for BadgeOptions {
    fn default() -> BadgeOptions {
        BadgeOptions {
            subject: "build".to_owned(),
            status: "passing".to_owned(),
            color: "#4c1".to_owned(),
        }
    }
}

struct BadgeStaticData {
    font: Font<'static>,
    scale: Scale,
    offset: Point<f32>,
}

static DATA: Lazy<BadgeStaticData> = Lazy::new(|| {
    let font = Font::try_from_bytes(FONT_DATA).expect("failed to parse font collection");

    let v_metrics = font.v_metrics(SCALE);
    let offset = point(0.0, v_metrics.ascent);

    BadgeStaticData {
        font,
        scale: SCALE,
        offset,
    }
});

pub struct Badge {
    options: BadgeOptions,
}

impl Badge {
    pub fn new(options: BadgeOptions) -> Badge {
        Badge { options }
    }

    pub fn to_svg_data_uri(&self) -> String {
        format!(
            "data:image/svg+xml;base64,{}",
            Base64Display::with_config(self.to_svg().as_bytes(), base64::STANDARD)
        )
    }

    pub fn to_svg(&self) -> String {
        let left_width = self.calculate_width(&self.options.subject) + 6;
        let right_width = self.calculate_width(&self.options.status) + 6;

        let svg = format!(
            r###"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="20">
  <linearGradient id="smooth" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>

  <mask id="round">
    <rect width="{}" height="20" rx="3" fill="#fff"/>
  </mask>

  <g mask="url(#round)">
    <rect width="{}" height="20" fill="#555"/>
    <rect x="{}" width="{}" height="20" fill="{}"/>
    <rect width="{}" height="20" fill="url(#smooth)"/>
  </g>

  <g fill="#fff" text-anchor="middle" font-family="DejaVu Sans,Verdana,Geneva,sans-serif" font-size="11">
    <text x="{}" y="15" fill="#010101" fill-opacity=".3">{}</text>
    <text x="{}" y="14">{}</text>
    <text x="{}" y="15" fill="#010101" fill-opacity=".3">{}</text>
    <text x="{}" y="14">{}</text>
  </g>
</svg>"###,
            left_width + right_width,
            left_width + right_width,
            left_width,
            left_width,
            right_width,
            self.options.color,
            left_width + right_width,
            (left_width) / 2,
            self.options.subject,
            (left_width) / 2,
            self.options.subject,
            left_width + (right_width / 2),
            self.options.status,
            left_width + (right_width / 2),
            self.options.status
        );

        svg
    }

    fn calculate_width(&self, text: &str) -> u32 {
        let glyphs: Vec<PositionedGlyph> =
            DATA.font.layout(text, DATA.scale, DATA.offset).collect();
        let width = glyphs
            .iter()
            .rev()
            .filter_map(|g| {
                g.pixel_bounding_box()
                    .map(|b| b.min.x as f32 + g.unpositioned().h_metrics().advance_width)
            })
            .next()
            .unwrap_or(0.0);
        (width + ((text.len() as f32 - 1f32) * 1.3)).ceil() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> BadgeOptions {
        BadgeOptions::default()
    }

    #[test]
    fn test_calculate_width() {
        let badge = Badge::new(options());
        assert_eq!(badge.calculate_width("build"), 29);
        assert_eq!(badge.calculate_width("passing"), 44);
    }

    #[test]
    #[ignore]
    fn test_to_svg() {
        use std::fs::File;
        use std::io::Write;
        let mut file = File::create("test.svg").unwrap();
        let options = BadgeOptions {
            subject: "build".to_owned(),
            status: "passing".to_owned(),
            ..BadgeOptions::default()
        };
        let badge = Badge::new(options);
        file.write_all(badge.to_svg().as_bytes()).unwrap();
    }
}

//! Simple badge generator

use std::sync::LazyLock;

use base64::display::Base64Display;
use rusttype::{point, Font, Point, PositionedGlyph, Scale};
use serde::Deserialize;

const FONT_DATA: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/DejaVuSans.ttf"));
const FONT_SIZE: f32 = 11.;
const SCALE: Scale = Scale {
    x: FONT_SIZE,
    y: FONT_SIZE,
};

/// Badge style name.
///
/// Default style is "flat".
///
/// Matches style names from shields.io.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BadgeStyle {
    #[default]
    Flat,
    FlatSquare,
    ForTheBadge,
}

#[derive(Debug, Clone)]
pub struct BadgeOptions {
    /// Subject will be displayed on the left side of badge
    pub subject: String,

    /// Status will be displayed on the right side of badge
    pub status: String,

    /// HTML color of badge
    pub color: String,

    /// Style of badge.
    pub style: BadgeStyle,
}

impl Default for BadgeOptions {
    fn default() -> BadgeOptions {
        BadgeOptions {
            subject: "build".to_owned(),
            status: "passing".to_owned(),
            color: "#4c1".to_owned(),
            style: BadgeStyle::Flat,
        }
    }
}

struct BadgeStaticData {
    font: Font<'static>,
    scale: Scale,
    offset: Point<f32>,
}

static DATA: LazyLock<BadgeStaticData> = LazyLock::new(|| {
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
            Base64Display::new(self.to_svg().as_bytes(), &base64::prelude::BASE64_STANDARD)
        )
    }

    pub fn to_svg(&self) -> String {
        match self.options.style {
            BadgeStyle::Flat => self.to_flat_svg(),
            BadgeStyle::FlatSquare => self.to_flat_square_svg(),
            BadgeStyle::ForTheBadge => self.to_for_the_badge_svg(),
        }
    }

    pub fn to_flat_svg(&self) -> String {
        let left_width = self.calculate_width(&self.options.subject) + 6;
        let right_width = self.calculate_width(&self.options.status) + 6;
        let total_width = left_width + right_width;

        let left_center = left_width / 2;
        let right_center = left_width + (right_width / 2);

        let color = &self.options.color;
        let subject = &self.options.subject;
        let status = &self.options.status;

        let svg = format!(
            r###"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{total_width}" height="20">
  <linearGradient id="smooth" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>

  <mask id="round">
    <rect width="{total_width}" height="20" rx="3" fill="#fff"/>
  </mask>

  <g mask="url(#round)">
    <rect width="{left_width}" height="20" fill="#555"/>
    <rect width="{right_width}" height="20" x="{left_width}" fill="{color}"/>
    <rect width="{total_width}" height="20" fill="url(#smooth)"/>
  </g>

  <g fill="#fff" text-anchor="middle" font-family="DejaVu Sans,Verdana,Geneva,sans-serif" font-size="11" text-rendering="geometricPrecision">
    <text x="{left_center}" y="15" fill="#010101" fill-opacity=".3">{subject}</text>
    <text x="{left_center}" y="14">{subject}</text>
    <text x="{right_center}" y="15" fill="#010101" fill-opacity=".3">{status}</text>
    <text x="{right_center}" y="14">{status}</text>
  </g>
</svg>"###
        );

        svg
    }

    pub fn to_flat_square_svg(&self) -> String {
        let left_width = self.calculate_width(&self.options.subject) + 6;
        let right_width = self.calculate_width(&self.options.status) + 6;
        let total_width = left_width + right_width;

        let left_center = left_width / 2;
        let right_center = left_width + (right_width / 2);

        let color = &self.options.color;
        let subject = &self.options.subject;
        let status = &self.options.status;

        let svg = format!(
            r###"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{total_width}" height="20" text-rendering="geometricPrecision">
  <g>
    <rect width="{left_width}" height="20" fill="#555"/>
    <rect width="{right_width}" height="20" x="{left_width}" fill="{color}"/>
  </g>

  <g fill="#fff" text-anchor="middle" font-family="DejaVu Sans,Verdana,Geneva,sans-serif" font-size="11">
    <text x="{left_center}" y="14">{subject}</text>
    <text x="{right_center}" y="14">{status}</text>
  </g>
</svg>
"###,
        );

        svg
    }

    pub fn to_for_the_badge_svg(&self) -> String {
        let left_width = self.calculate_width(&self.options.subject) + 38;
        let right_width = self.calculate_width(&self.options.status) + 38;
        let total_width = left_width + right_width;

        let left_center = left_width / 2;
        let right_center = left_width + (right_width / 2);

        let color = &self.options.color;
        let subject = self.options.subject.to_uppercase();
        let status = self.options.status.to_uppercase();

        let svg = format!(
            r###"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{total_width}" height="28">
  <g>
    <rect width="{left_width}" height="28" fill="#555"/>
    <rect width="{right_width}" height="28" x="{left_width}" fill="{color}"/>
  </g>

  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="10" text-rendering="geometricPrecision">
    <text x="{left_center}" y="18" letter-spacing="1">{subject}</text>
    <text x="{right_center}" y="18" font-weight="bold" letter-spacing="1">{status}</text>
  </g>
</svg>
"###,
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
        use std::{fs::File, io::Write as _};

        let mut file = File::create("test.svg").unwrap();
        let options = BadgeOptions {
            subject: "latest".to_owned(),
            status: "v4.0.0-beta.21".to_owned(),
            style: BadgeStyle::ForTheBadge,
            color: "#fe7d37".to_owned(),
        };
        let badge = Badge::new(options);
        file.write_all(badge.to_svg().as_bytes()).unwrap();
    }

    #[test]
    fn deserialize_badge_style() {
        #[derive(Debug, Deserialize)]
        struct Foo {
            style: BadgeStyle,
        }

        let style = serde_urlencoded::from_str::<Foo>("style=flat").unwrap();
        assert_eq!(style.style, BadgeStyle::Flat);

        let style = serde_urlencoded::from_str::<Foo>("style=flat-square").unwrap();
        assert_eq!(style.style, BadgeStyle::FlatSquare);
    }
}

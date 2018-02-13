//pub mod templates;

pub static BADGE_UPTODATE_SVG: &'static [u8; 975] =
    include_bytes!("../../assets/badges/up-to-date.svg");
pub static BADGE_OUTDATED_SVG: &'static [u8; 974] =
    include_bytes!("../../assets/badges/outdated.svg");
pub static BADGE_UNKNOWN_SVG: &'static [u8; 972] =
    include_bytes!("../../assets/badges/unknown.svg");

pub static STATIC_STYLE_CSS: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/style.css"));
pub static STATIC_FAVICON_PNG: &'static [u8; 1338] =
    include_bytes!("../../assets/favicon.png");

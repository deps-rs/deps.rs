//pub mod templates;

pub static BADGE_UPTODATE_SVG: &'static [u8; 978] =
    include_bytes!("../../assets/badges/up-to-date.svg");
pub static BADGE_OUTDATED_SVG: &'static [u8; 974] =
    include_bytes!("../../assets/badges/outdated.svg");

pub static STATIC_STYLE_CSS: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/style.css"));

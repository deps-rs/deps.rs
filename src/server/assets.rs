pub static STATIC_STYLE_CSS: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/style.css"));
pub static STATIC_FAVICON_PNG: &'static [u8; 1338] =
    include_bytes!("../../assets/favicon.png");

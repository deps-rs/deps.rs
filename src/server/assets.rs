pub static STATIC_STYLE_CSS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/style.css"));
pub const STATIC_STYLE_CSS_PATH: &str = concat!(
    "/static/style.",
    include_str!(concat!(env!("OUT_DIR"), "/style.css.sha1")),
    ".css"
);
pub const STATIC_STYLE_CSS_ETAG: &str = concat!(
    "\"",
    include_str!(concat!(env!("OUT_DIR"), "/style.css.sha1")),
    "\""
);
pub static STATIC_FAVICON: &[u8] = include_bytes!("../../assets/logo.svg");

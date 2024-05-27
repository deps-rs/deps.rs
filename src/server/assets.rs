pub static STATIC_STYLE_CSS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/style.css"));
pub const STATIC_STYLE_CSS_PATH: &str = concat!(
    "/static/style.",
    include_str!(concat!(env!("OUT_DIR"), "/style.css.sha1")),
    ".css"
);
pub const STATIC_STYLE_CSS_ETAG: &str = include_str!(concat!(env!("OUT_DIR"), "/style.css.sha1"));

pub const STATIC_FAVICON_PATH: &str = "/static/logo.svg";
pub static STATIC_FAVICON: &[u8] = include_bytes!("../../assets/logo.svg");

pub static STATIC_LINKS_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/links.js"));
pub const STATIC_LINKS_JS_PATH: &str = concat!(
    "/static/links.",
    include_str!(concat!(env!("OUT_DIR"), "/links.js.sha1")),
    ".js"
);
pub const STATIC_LINKS_JS_ETAG: &str = include_str!(concat!(env!("OUT_DIR"), "/links.js.sha1"));

extern crate sass_rs as sass;

use std::env;
use std::fs;
use std::path::Path;

use sha1::{Digest, Sha1};

fn build_style() -> String {
    let options = sass::Options {
        output_style: sass::OutputStyle::Compressed,
        ..Default::default()
    };

    sass::compile_file("./assets/styles/main.sass", options).expect("failed to compile style sheet")
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let style = build_style();

    let css_path = Path::new(&out_dir).join("style.css");
    fs::write(css_path, style.as_bytes()).unwrap();

    let hash_path = Path::new(&out_dir).join("style.css.sha1");
    let digest = Sha1::digest(style.as_bytes());
    fs::write(hash_path, format!("{:x}", digest)).unwrap();
}

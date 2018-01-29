extern crate sass_rs as sass;

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn build_style() -> String {
    let options = sass::Options {
        output_style: sass::OutputStyle::Compressed,
        ..Default::default()
    };

    sass::compile_file("./assets/styles/main.sass", options)
        .expect("failed to compile style sheet")
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("style.css");
    let mut f = File::create(&dest_path).unwrap();

    f.write_all(build_style().as_bytes()).unwrap();
}

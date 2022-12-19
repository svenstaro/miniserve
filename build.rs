use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR env var missing");
    let dest_path = Path::new(&out_dir).join("style.css");
    fs::write(
        dest_path,
        grass::from_string(
            include_str!("data/style.scss").to_string(),
            &grass::Options::default(),
        )
        .expect("scss failed to compile"),
    )
    .expect("failed to write css file");

    println!("cargo:rerun-if-changed=data/style.scss");
}

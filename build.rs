use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("style.css");
    fs::write(
        &dest_path,
        grass::from_string(include_str!("data/style.scss").to_string(), &grass::Options::default()).unwrap()
    ).unwrap();
    println!("cargo:rerun-if-changed=data/style.css");
}

use std::error::Error;
use std::path::PathBuf;
use std::{env, fs};

use sasso::{FsImporter, Options};

fn main() -> Result<(), Box<dyn Error>> {
    let data = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("data");
    let out = PathBuf::from(env::var("OUT_DIR")?);

    println!("cargo:rerun-if-changed=data");

    let importer = FsImporter::new(vec![data.to_path_buf()]);
    let options = Options::default().with_importer(&importer);

    let mut sheets = vec![data.join("style.scss")];
    for theme in fs::read_dir(data.join("themes"))? {
        sheets.push(theme?.path());
    }

    for sheet in sheets {
        let Some(name) = sheet.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        let css = sasso::compile(&fs::read_to_string(&sheet)?, &options)?;
        fs::write(out.join(format!("{name}.css")), css)?;
    }

    Ok(())
}

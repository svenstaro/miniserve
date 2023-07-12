use std::env;
use std::fs;
use std::path::Path;

const THEMES: &[&str] = &["squirrel", "archlinux", "zenburn", "monokai"];

fn main() {
    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR env var missing");

    let dest_path = Path::new(&out_dir).join("style.css");
    fs::write(
        dest_path,
        grass::from_path("data/style.scss", &grass::Options::default())
            .expect("scss failed to compile"),
    )
    .expect("failed to write css file");

    for theme in THEMES.iter() {
        let dest_path = Path::new(&out_dir).join(format!("theme-{}.css", theme));
        fs::write(
            dest_path,
            grass::from_string(
                format!(
                    r#"
                @use "data/themes/{theme}";
                body:not([data-theme]) {{
                    @include {theme}.theme();
                }}
                "#,
                    theme = theme
                ),
                &grass::Options::default(),
            )
            .expect("scss failed to compile"),
        )
        .expect("failed to write css file");
    }

    println!("cargo:rerun-if-changed=data/style.scss");
}

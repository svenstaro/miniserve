mod fixtures;

use fixtures::{server, Error, TestServer, DIRECTORIES, FILES};
use rstest::rstest;
use select::predicate::Attr;
use select::{document::Document, node::Node};
use std::fs::{remove_file, File};
use std::io::Write;

#[rstest]
/// Do not show readme contents by default
fn no_readme_contents(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
    }
    for &dir in DIRECTORIES {
        assert!(parsed.find(|x: &Node| x.text() == dir).next().is_some());
    }
    assert!(parsed.find(Attr("id", "readme")).next().is_none());
    assert!(parsed.find(Attr("id", "readme-filename")).next().is_none());
    assert!(parsed.find(Attr("id", "readme-contents")).next().is_none());

    Ok(())
}

#[rstest(
    readme_name,
    case("Readme.md"),
    case("readme.md"),
    case("README.md"),
    case("README.MD"),
    case("ReAdMe.Md")
)]
/// Show readme contents when told to if there is readme.md/README.md file
fn show_readme_contents(
    #[with(&["--readme"])] server: TestServer,
    readme_name: &str,
) -> Result<(), Error> {
    for dir in DIRECTORIES {
        let readme_path = server.path().join(dir).join(readme_name);
        let mut readme_file = File::create(&readme_path)?;
        readme_file
            .write(
                format!("Contents of {}", readme_name)
                    .to_string()
                    .as_bytes(),
            )
            .expect("Couldn't write readme");
        let body = reqwest::blocking::get(server.url().join(dir)?)?.error_for_status()?;
        let parsed = Document::from_read(body)?;

        for &file in FILES {
            assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
        }

        assert!(parsed.find(Attr("id", "readme")).next().is_some());
        assert!(parsed.find(Attr("id", "readme-filename")).next().is_some());
        assert!(
            parsed
                .find(Attr("id", "readme-filename"))
                .next()
                .unwrap()
                .text()
                == readme_name
        );
        assert!(parsed.find(Attr("id", "readme-contents")).next().is_some());
        assert!(
            parsed
                .find(Attr("id", "readme-contents"))
                .next()
                .unwrap()
                .text()
                .trim()
                == format!("Contents of {}", readme_name)
        );
        remove_file(readme_path).unwrap();
    }
    Ok(())
}

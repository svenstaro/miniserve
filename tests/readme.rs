mod fixtures;

use fixtures::{server, Error, TestServer, DIRECTORIES, FILES};
use rstest::rstest;
use select::predicate::Attr;
use select::{document::Document, node::Node};
use std::fs::{create_dir_all, File};
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
    makedir,
    dir,
    content,
    case(false, "", "Test Hello Yes"),
    case(false, "/dira/", "This is dira/readme.md"),
    case(false, "/dirb/", "This is dirb/readme.md"),
    case(true, "/readme-dira/", "This is readme-dira/README.md")
)]
/// Show readme contents when told to if there is readme.md/README.md file
fn show_readme_contents(
    #[with(&["--readme"])] server: TestServer,
    makedir: bool,
    dir: &str,
    content: &str,
) -> Result<(), Error> {
    if makedir {
        let tempdir = server.path().join(dir.strip_prefix("/").unwrap());
        create_dir_all(tempdir.clone()).unwrap();
        let mut readme_file = File::create(tempdir.join("README.md"))?;
        readme_file
            .write(content.to_string().as_bytes())
            .expect("Couldn't write README.md");
    }

    let body = reqwest::blocking::get(server.url().join(dir)?)?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    if !makedir {
        for &file in FILES {
            assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
        }
    }

    assert!(parsed.find(Attr("id", "readme")).next().is_some());
    assert!(parsed.find(Attr("id", "readme-filename")).next().is_some());
    assert!(
        parsed
            .find(Attr("id", "readme-filename"))
            .next()
            .unwrap()
            .text()
            .to_lowercase()
            == "readme.md"
    );
    assert!(parsed.find(Attr("id", "readme-contents")).next().is_some());
    assert!(
        parsed
            .find(Attr("id", "readme-contents"))
            .next()
            .unwrap()
            .text()
            .trim()
            == content
    );

    Ok(())
}

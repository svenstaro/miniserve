mod fixtures;

use fixtures::{server, Error, TestServer, DIRECTORIES, FILES};
use rstest::rstest;
use select::predicate::Attr;
use select::{document::Document, node::Node};

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
    dir,
    content,
    case("", "Test Hello Yes"),
    case("/dira/", "This is dira/readme.md"),
    case("/dirb/", "This is dirb/readme.md")
)]
/// Show readme contents when told to if there is readme.md file
fn show_readme_contents(
    #[with(&["--readme"])] server: TestServer,
    dir: &str,
    content: &str,
) -> Result<(), Error> {
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

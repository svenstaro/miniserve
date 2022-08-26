mod fixtures;

use fixtures::{server, Error, TestServer, DIRECTORIES};
use rstest::rstest;
use select::document::Document;
use select::predicate::Attr;

#[rstest]
/// Do not show readme contents by default
fn no_readme_contents(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Attr("id", "readme")).next().is_none());
    assert!(parsed.find(Attr("id", "readme-filename")).next().is_none());
    assert!(parsed.find(Attr("id", "readme-contents")).next().is_none());

    Ok(())
}

#[rstest]
/// Show readme contents when told to if there is readme.md file
fn show_readme_contents(#[with(&["--readme"])] server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
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
            == "Test Hello Yes"
    );

    Ok(())
}

#[rstest]
/// Show readme contents when told to if there is readme.md file on directories.
fn show_readme_contents_directories(#[with(&["--readme"])] server: TestServer) -> Result<(), Error> {
    let directories = DIRECTORIES.to_vec();

    for directory in directories {
        let dir_body =
            reqwest::blocking::get(server.url().join(&directory)?)?
                .error_for_status()?;
        let dir_body_parsed = Document::from_read(dir_body)?;
        assert!(dir_body_parsed.find(Attr("id", "readme")).next().is_some());
        assert!(dir_body_parsed
            .find(Attr("id", "readme-filename"))
            .next()
            .is_some());
        assert!(
            dir_body_parsed
                .find(Attr("id", "readme-filename"))
                .next()
                .unwrap()
                .text()
                == "readme.md"
        );
        assert!(dir_body_parsed
            .find(Attr("id", "readme-contents"))
            .next()
            .is_some());
        assert!(
            dir_body_parsed
                .find(Attr("id", "readme-contents"))
                .next()
                .unwrap()
                .text()
                .trim()
                == &format!("This is {}readme.md", directory)
        );
    }

    Ok(())
}

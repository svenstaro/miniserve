mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::{FileWriteStr, TempDir};
use assert_fs::prelude::PathChild;
use fixtures::{server, tmpdir, Error, TestServer, DIRECTORIES};
use reqwest::Url;
use rstest::rstest;
use select::document::Document;
use select::predicate::Attr;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

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
fn show_readme_contents(tmpdir: TempDir) -> Result<(), Error> {
    tmpdir
        .child("readme.md")
        .write_str("Readme Contents.")
        .expect("Couldn't write to readme.md");
    let mut child = Command::cargo_bin("miniserve")?
        .arg("--readme")
        .arg("--port")
        .arg("8090")
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));
    let body = reqwest::blocking::get("http://localhost:8090")?.error_for_status()?;
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
            == "Readme Contents."
    );

    child.kill()?;
    Ok(())
}

#[rstest]
/// Show readme contents when told to if there is readme.md file on directories.
fn show_readme_contents_directories(tmpdir: TempDir) -> Result<(), Error> {
    let directories = DIRECTORIES.to_vec();
    for directory in directories.iter() {
        tmpdir
            .child(format!("{}{}", directory, "readme.md"))
            .write_str(&format!("Readme Contents for {}.", directory))
            .expect("Couldn't write to file");
    }

    let mut child = Command::cargo_bin("miniserve")?
        .arg("--readme")
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    for directory in directories {
        let dir_body =
            reqwest::blocking::get(Url::parse("http://localhost:8080")?.join(&directory)?)?
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
                == format!("Readme Contents for {}.", directory)
        );
    }

    child.kill()?;
    Ok(())
}

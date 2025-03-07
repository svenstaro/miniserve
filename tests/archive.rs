use reqwest::StatusCode;
use rstest::rstest;
use select::{document::Document, predicate::Text};

mod fixtures;

use crate::fixtures::{Error, TestServer, server};

#[rstest]
fn archives_are_disabled(server: TestServer) -> Result<(), Error> {
    // Ensure the links to the archives are not present
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(
        parsed
            .find(Text)
            .all(|x| x.text() != "Download .tar.gz" && x.text() != "Download .tar")
    );

    // Try to download anyway, ensure it's forbidden
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar_gz")?)?.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar")?)?.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=zip")?)?.status(),
        StatusCode::FORBIDDEN
    );

    Ok(())
}

#[rstest]
fn test_tar_archives(#[with(&["-g"])] server: TestServer) -> Result<(), Error> {
    // Ensure the links to the tar archive exists and tar not exists
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == "Download .tar.gz"));
    assert!(parsed.find(Text).all(|x| x.text() != "Download .tar"));

    // Try to download, only tar_gz should works
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar_gz")?)?.status(),
        StatusCode::OK
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar")?)?.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=zip")?)?.status(),
        StatusCode::FORBIDDEN
    );

    Ok(())
}

#[rstest]
fn archives_are_disabled_when_indexing_disabled(
    #[with(&["--disable-indexing", "--enable-tar-gz", "--enable-tar", "--enable-zip"])]
    server: TestServer,
) -> Result<(), Error> {
    // Ensure the links to the archives are not present
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(
        parsed
            .find(Text)
            .all(|x| x.text() != "Download .tar.gz" && x.text() != "Download .tar")
    );

    // Try to download anyway, ensure it's forbidden
    // We assert for not found to make sure we aren't leaking information about directories that do exist.
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar_gz")?)?.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar")?)?.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=zip")?)?.status(),
        StatusCode::NOT_FOUND
    );

    Ok(())
}

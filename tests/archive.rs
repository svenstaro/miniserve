use reqwest::{StatusCode, blocking::Client};
use rstest::rstest;
use select::{document::Document, predicate::Text};

mod fixtures;

use crate::fixtures::{Error, TestServer, reqwest_client, server};

#[rstest]
fn archives_are_disabled(server: TestServer, reqwest_client: Client) -> Result<(), Error> {
    // Ensure the links to the archives are not present
    let body = reqwest_client
        .get(server.url())
        .send()?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(
        parsed
            .find(Text)
            .all(|x| x.text() != "Download .tar.gz" && x.text() != "Download .tar")
    );

    // Try to download anyway, ensure it's forbidden
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=tar_gz")?)
            .send()?
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=tar")?)
            .send()?
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=zip")?)
            .send()?
            .status(),
        StatusCode::FORBIDDEN
    );

    Ok(())
}

#[rstest]
fn test_tar_archives(
    #[with(&["-g"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    // Ensure the links to the tar archive exists and tar not exists
    let body = reqwest_client
        .get(server.url())
        .send()?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == "Download .tar.gz"));
    assert!(parsed.find(Text).all(|x| x.text() != "Download .tar"));

    // Try to download, only tar_gz should works
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=tar_gz")?)
            .send()?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=tar")?)
            .send()?
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=zip")?)
            .send()?
            .status(),
        StatusCode::FORBIDDEN
    );

    Ok(())
}

#[rstest]
fn archives_are_disabled_when_indexing_disabled(
    #[with(&["--disable-indexing", "--enable-tar-gz", "--enable-tar", "--enable-zip"])]
    server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    // Ensure the links to the archives are not present
    let body = reqwest_client.get(server.url()).send()?;
    let parsed = Document::from_read(body)?;
    assert!(
        parsed
            .find(Text)
            .all(|x| x.text() != "Download .tar.gz" && x.text() != "Download .tar")
    );

    // Try to download anyway, ensure it's forbidden
    // We assert for not found to make sure we aren't leaking information about directories that do exist.
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=tar_gz")?)
            .send()?
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=tar")?)
            .send()?
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        reqwest_client
            .get(server.url().join("?download=zip")?)
            .send()?
            .status(),
        StatusCode::NOT_FOUND
    );

    Ok(())
}

use reqwest::{StatusCode, blocking::Client};
use rstest::rstest;
use select::{document::Document, predicate::Text};
use std::io::Cursor;
use zip::ZipArchive;

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
    // Ensure the links to the tar.gz archive exists and tar and zip not exists
    let body = reqwest_client
        .get(server.url())
        .send()?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == "Download .tar.gz"));
    assert!(parsed.find(Text).all(|x| x.text() != "Download .tar"));
    assert!(parsed.find(Text).all(|x| x.text() != "Download .zip"));

    // Try to download, only tar_gz should work
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

#[rstest]
fn archive_behave_differently_with_broken_symlinks(
    #[with(&["--enable-tar-gz", "--enable-tar", "--enable-zip"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let download_archive = |download_type: &str| {
        let resp = reqwest_client
            .get(
                server
                    .url()
                    .join(format!("?download={}", download_type).as_str())
                    .unwrap(),
            )
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        resp.bytes().unwrap()
    };

    // Produce a file with only partial header fields. See "rfc1952 § 2.3.1. Member header and trailer".
    // ArchiveCreationError("tarball", IoError("Failed to append the content of ... to the TAR archive"
    {
        let bytes = download_archive("tar_gz");
        assert_eq!(bytes.len(), 10);
    }

    // Produce a tarball containing a subset of files
    // ArchiveCreationError("tarball", IoError("Failed to append the content of ... to the TAR archive"
    {
        let bytes = download_archive("tar");
        assert!(bytes.len() >= 512 + 512 + 2 * 512); // at least: header block + data + end marker
    }

    // Produce an empty file
    // Error during archive creation: ArchiveCreationError("zip", ArchiveCreationError(
    //   "Failed to create the ZIP archive", IoError("Could not get file metadata ..."
    {
        let bytes = download_archive("zip");
        assert_eq!(bytes.len(), 0);
    }

    Ok(())
}

#[rstest]
fn zip_archives_store_entry_name_in_unix_style(
    #[with(&["--enable-zip"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let resp = reqwest_client
        .get(server.url().join("someDir/?download=zip")?)
        .send()?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::OK);

    let mut archive = ZipArchive::new(Cursor::new(resp.bytes()?))?;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name();

        // Assert that the name does not contain any backslashes
        assert!(
            !name.contains(r"\"),
            "ZIP entry '{}' contains a backslash",
            name
        );
    }

    Ok(())
}

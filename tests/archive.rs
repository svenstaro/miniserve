use reqwest::StatusCode;
use rstest::rstest;
use select::{document::Document, predicate::Text};
use std::io::Cursor;
use zip;

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

#[rstest]
fn archive_behave_differently_with_broken_symlinks(
    #[with(&["--enable-tar-gz", "--enable-tar", "--enable-zip"])] server: TestServer,
) -> Result<(), Error> {
    let download_archive = |download_type: &str| {
        let body = reqwest::blocking::get(
            server
                .url()
                .join(format!("?download={}", download_type).as_str())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body.status(), StatusCode::OK);
        body.bytes().unwrap()
    };

    // Produce a file with only partial header fields. See "rfc1952 § 2.3.1. Member header and trailer".
    // ArchiveCreationError("tarball", IoError("Failed to append the content of ... to the TAR archive",
    //   Os { code: 1921, kind: FilesystemLoop, message: "The name of the file cannot be resolved by the system." }))
    {
        let bytes = download_archive("tar_gz");
        assert_eq!(bytes.len(), 10);
    }

    // Produce an incomplete file
    // ArchiveCreationError("tarball", IoError("Failed to append the content of ... to the TAR archive",
    //   Os { code: 1921, kind: FilesystemLoop, message: "The name of the file cannot be resolved by the system." }))
    {
        let bytes = download_archive("tar");
        assert_eq!(bytes.len(), 51200);
    }

    // Produce an empty file
    // Error during archive creation: ArchiveCreationError("zip", ArchiveCreationError(
    //   "Failed to create the ZIP archive", IoError("Could not get file metadata ...",
    //   Os { code: 1921, kind: FilesystemLoop, message: "The name of the file cannot be resolved by the system." })))
    {
        let bytes = download_archive("zip");
        assert_eq!(bytes.len(), 0);
    }

    Ok(())
}

#[rstest]
fn zip_archives_store_entry_name_in_unix_style(
    #[with(&["--enable-zip"])] server: TestServer,
) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url().join("someDir/?download=zip")?)?;
    assert_eq!(body.status(), StatusCode::OK);

    let mut archive = zip::ZipArchive::new(Cursor::new(body.bytes()?))?;
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

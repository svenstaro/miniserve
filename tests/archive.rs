use std::io::Cursor;

use reqwest::{StatusCode, blocking::Client};
use rstest::rstest;
use select::{document::Document, predicate::Text};
use zip::ZipArchive;

mod fixtures;

use crate::fixtures::{Error, TestServer, reqwest_client, server};

enum ArchiveKind {
    TarGz,
    Tar,
    Zip,
}

impl ArchiveKind {
    fn server_option(&self) -> &'static str {
        match self {
            ArchiveKind::TarGz => "--enable-tar-gz",
            ArchiveKind::Tar => "--enable-tar",
            ArchiveKind::Zip => "--enable-zip",
        }
    }

    fn link_text(&self) -> &'static str {
        match self {
            ArchiveKind::TarGz => "Download .tar.gz",
            ArchiveKind::Tar => "Download .tar",
            ArchiveKind::Zip => "Download .zip",
        }
    }

    fn download_param(&self) -> &'static str {
        match self {
            ArchiveKind::TarGz => "?download=tar_gz",
            ArchiveKind::Tar => "?download=tar",
            ArchiveKind::Zip => "?download=zip",
        }
    }
}

fn fetch_index_document(
    reqwest_client: &Client,
    server: &TestServer,
    expected: StatusCode,
) -> Result<Document, Error> {
    let resp = reqwest_client.get(server.url()).send()?;
    assert_eq!(resp.status(), expected);

    Ok(Document::from_read(resp)?)
}

fn download_archive_bytes(
    reqwest_client: &Client,
    server: &TestServer,
    kind: ArchiveKind,
) -> Result<(StatusCode, usize), Error> {
    let resp = reqwest_client
        .get(server.url().join(kind.download_param())?)
        .send()?;

    Ok((resp.status(), resp.bytes()?.len()))
}

fn assert_link_presence(document: &Document, present: &[&str], absent: &[&str]) {
    let contains_text =
        |document: &Document, text: &str| document.find(Text).any(|x| x.text() == text);

    for text in present {
        assert!(
            contains_text(document, text),
            "Expected link text '{text}' to be present",
        );
    }

    for text in absent {
        assert!(
            !contains_text(document, text),
            "Expected link text '{text}' to be absent",
        );
    }
}

/// By default, all archive links are hidden.
#[rstest]
fn archives_are_disabled_links(server: TestServer, reqwest_client: Client) -> Result<(), Error> {
    let document = fetch_index_document(&reqwest_client, &server, StatusCode::OK)?;
    assert_link_presence(
        &document,
        &[],
        &[
            ArchiveKind::TarGz.link_text(),
            ArchiveKind::Tar.link_text(),
            ArchiveKind::Zip.link_text(),
        ],
    );

    Ok(())
}

/// By default, downloading archives is forbidden.
#[rstest]
#[case(ArchiveKind::TarGz)]
#[case(ArchiveKind::Tar)]
#[case(ArchiveKind::Zip)]
fn archives_are_disabled_downloads(
    #[case] kind: ArchiveKind,
    server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let (status_code, _) = download_archive_bytes(&reqwest_client, &server, kind)?;
    assert_eq!(status_code, StatusCode::FORBIDDEN);

    Ok(())
}

/// When indexing is disabled, archive links are hidden despite enabled archive options.
#[rstest]
fn archives_are_disabled_when_indexing_disabled_links(
    #[with(&["--disable-indexing", "--enable-tar-gz", "--enable-tar", "--enable-zip"])]
    server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let document = fetch_index_document(&reqwest_client, &server, StatusCode::NOT_FOUND)?;
    assert_link_presence(
        &document,
        &[],
        &[
            ArchiveKind::TarGz.link_text(),
            ArchiveKind::Tar.link_text(),
            ArchiveKind::Zip.link_text(),
        ],
    );

    Ok(())
}

/// When indexing is disabled, archive downloads are not found despite enabled archive options.
#[rstest]
#[case(ArchiveKind::TarGz)]
#[case(ArchiveKind::Tar)]
#[case(ArchiveKind::Zip)]
fn archives_are_disabled_when_indexing_disabled_downloads(
    #[case] kind: ArchiveKind,
    #[with(&["--disable-indexing", "--enable-tar-gz", "--enable-tar", "--enable-zip"])]
    server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let (status_code, _) = download_archive_bytes(&reqwest_client, &server, kind)?;
    assert_eq!(status_code, StatusCode::NOT_FOUND);

    Ok(())
}

/// Ensure the link and download to the specified archive is available and others are not
#[rstest]
#[case::tar_gz(ArchiveKind::TarGz)]
#[case::tar(ArchiveKind::Tar)]
#[case::zip(ArchiveKind::Zip)]
fn archives_links_and_downloads(
    #[case] kind: ArchiveKind,
    #[with(&[kind.server_option()])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let document = fetch_index_document(&reqwest_client, &server, StatusCode::OK)?;

    let (link_text, other_links, tar_gz_status, tar_status, zip_status) = match kind {
        ArchiveKind::TarGz => (
            ArchiveKind::TarGz.link_text(),
            [ArchiveKind::Tar.link_text(), ArchiveKind::Zip.link_text()],
            StatusCode::OK,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
        ),
        ArchiveKind::Tar => (
            ArchiveKind::Tar.link_text(),
            [ArchiveKind::TarGz.link_text(), ArchiveKind::Zip.link_text()],
            StatusCode::FORBIDDEN,
            StatusCode::OK,
            StatusCode::FORBIDDEN,
        ),
        ArchiveKind::Zip => (
            ArchiveKind::Zip.link_text(),
            [ArchiveKind::TarGz.link_text(), ArchiveKind::Tar.link_text()],
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            StatusCode::OK,
        ),
    };

    assert_link_presence(&document, &[link_text], &other_links);

    for (kind, expected) in [
        (ArchiveKind::TarGz, tar_gz_status),
        (ArchiveKind::Tar, tar_status),
        (ArchiveKind::Zip, zip_status),
    ] {
        let (status, _) = download_archive_bytes(&reqwest_client, &server, kind)?;
        assert_eq!(status, expected);
    }

    Ok(())
}

enum ExpectedLen {
    /// Exact byte length expected.
    Exact(usize),
    /// Minimum byte length expected.
    Min(usize),
}

/// Broken symlinks (from [`fixtures::BROKEN_SYMLINK`]) yield different archive behaviors:
/// - tar_gz: a file with only partial header fields. See "rfc1952 § 2.3.1. Member header and trailer".
/// - tar: a tarball containing a subset of files.
/// - zip: an empty file.
#[rstest]
#[case::tar_gz(ArchiveKind::TarGz, ExpectedLen::Exact(10))]
#[case::tar(ArchiveKind::Tar, ExpectedLen::Min(512 + 512 + 2 * 512))]
#[case::zip(ArchiveKind::Zip, ExpectedLen::Exact(0))]
fn archive_behave_differently_with_broken_symlinks(
    #[case] kind: ArchiveKind,
    #[case] expected: ExpectedLen,
    #[with(&[ArchiveKind::TarGz.server_option(), ArchiveKind::Tar.server_option(), ArchiveKind::Zip.server_option()])]
    server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let (status_code, byte_len) = download_archive_bytes(&reqwest_client, &server, kind)?;
    assert_eq!(status_code, StatusCode::OK);

    match expected {
        ExpectedLen::Exact(len) => assert_eq!(byte_len, len),
        ExpectedLen::Min(len) => assert!(byte_len >= len),
    }

    Ok(())
}

/// ZIP archives store entry names using unix-style paths (no backslashes).
/// The "someDir" dir is constructed by [`fixtures`] and all items in it can be correctly processed.
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
        let name = entry.name()?;

        assert!(
            !name.contains(r"\"),
            "ZIP entry '{}' contains a backslash",
            name
        );
    }

    Ok(())
}

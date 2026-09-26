use std::io::{Cursor, Read};
use std::path::Path;

use reqwest::{StatusCode, blocking::Client};
use rstest::rstest;
use select::{document::Document, predicate::Text};
use zip::ZipArchive;

mod fixtures;

use crate::fixtures::{Error, TestServer, reqwest_client, server};

#[derive(Clone, Copy)]
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

/// Broken symlinks (from [`fixtures::BROKEN_SYMLINK`]) are omitted from the
/// archive rather than aborting the whole download. The remaining regular
/// files must still produce a non-trivial payload for every archive format.
#[rstest]
#[case::tar_gz(ArchiveKind::TarGz)]
#[case::tar(ArchiveKind::Tar)]
#[case::zip(ArchiveKind::Zip)]
fn archive_skips_broken_symlinks_and_still_succeeds(
    #[case] kind: ArchiveKind,
    #[with(&[ArchiveKind::TarGz.server_option(), ArchiveKind::Tar.server_option(), ArchiveKind::Zip.server_option()])]
    server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let (status_code, byte_len) = download_archive_bytes(&reqwest_client, &server, kind)?;
    assert_eq!(status_code, StatusCode::OK);
    // Each format must produce a real archive with the fixture files, not an
    // empty/truncated stream from an error mid-walk.
    assert!(
        byte_len > 64,
        "expected a non-trivial {} archive, got {byte_len} bytes",
        kind.link_text()
    );

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
        let name = entry.name();

        assert!(
            !name.contains(r"\"),
            "ZIP entry '{}' contains a backslash",
            name
        );
    }

    Ok(())
}

const OUTSIDE_SECRET: &str = "MINISERVE-OUTSIDE-SECRET\n";
const ESCAPE_LINK_NAME: &str = "link_to_outside";

/// Plant a symlink inside the served tree that points at a file *outside* it.
fn plant_escape_symlink(server: &TestServer) -> Result<(), Error> {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_file as symlink;

    // Sibling of the served root so a relative `../…` target resolves outside
    // the archive root. Unique name avoids clobbering parallel tests.
    let served = server.path();
    let durable_outside = served.parent().unwrap().join(format!(
        "outside-secret-{}.txt",
        served.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&durable_outside, OUTSIDE_SECRET)?;

    let link_path = served.join(ESCAPE_LINK_NAME);
    let relative_target = Path::new("..").join(durable_outside.file_name().unwrap());
    symlink(&relative_target, &link_path)?;

    Ok(())
}

/// Regression for #1568: ZIP generation used followed `metadata()`, so
/// `is_symlink()` never fired and `--no-symlinks` failed to stop packaging
/// the target of an escape symlink.
#[rstest]
#[case::zip_no_symlinks(
    ArchiveKind::Zip,
    &["--enable-zip", "--no-symlinks"]
)]
#[case::tar_no_symlinks(
    ArchiveKind::Tar,
    &["--enable-tar", "--no-symlinks"]
)]
#[case::tar_gz_no_symlinks(
    ArchiveKind::TarGz,
    &["--enable-tar-gz", "--no-symlinks"]
)]
fn archive_with_no_symlinks_omits_escape_symlink(
    #[case] kind: ArchiveKind,
    #[case] _args: &[&str],
    #[with(_args)] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    plant_escape_symlink(&server)?;

    let resp = reqwest_client
        .get(server.url().join(kind.download_param())?)
        .send()?
        .error_for_status()?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.bytes()?;

    match kind {
        ArchiveKind::Zip => {
            let mut archive = ZipArchive::new(Cursor::new(bytes.as_ref()))?;
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)?;
                let name = entry.name().to_string();
                assert!(
                    !name.contains(ESCAPE_LINK_NAME),
                    "ZIP must not contain escape symlink entry '{name}' with --no-symlinks"
                );
                let mut contents = String::new();
                // Only try to read regular file entries.
                if entry.is_file() {
                    entry.read_to_string(&mut contents)?;
                    assert!(
                        !contents.contains("MINISERVE-OUTSIDE-SECRET"),
                        "ZIP must not contain outside-root secret contents"
                    );
                }
            }
        }
        ArchiveKind::Tar | ArchiveKind::TarGz => {
            assert_tar_has_no_escape_secret(&bytes, matches!(kind, ArchiveKind::TarGz))?;
        }
    }

    Ok(())
}

/// Even when symlinks are allowed, archive generation must not package the
/// *contents* of a target that resolves outside the served root.
#[rstest]
#[case::zip(ArchiveKind::Zip, &["--enable-zip"])]
#[case::tar(ArchiveKind::Tar, &["--enable-tar"])]
#[case::tar_gz(ArchiveKind::TarGz, &["--enable-tar-gz"])]
fn archive_never_packages_outside_root_symlink_target(
    #[case] kind: ArchiveKind,
    #[case] _args: &[&str],
    #[with(_args)] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    plant_escape_symlink(&server)?;

    let resp = reqwest_client
        .get(server.url().join(kind.download_param())?)
        .send()?
        .error_for_status()?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.bytes()?;

    match kind {
        ArchiveKind::Zip => {
            let mut archive = ZipArchive::new(Cursor::new(bytes.as_ref()))?;
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)?;
                if entry.is_file() {
                    let mut contents = String::new();
                    entry.read_to_string(&mut contents)?;
                    assert!(
                        !contents.contains("MINISERVE-OUTSIDE-SECRET"),
                        "ZIP entry '{}' leaked outside-root secret",
                        entry.name()
                    );
                }
            }
        }
        ArchiveKind::Tar | ArchiveKind::TarGz => {
            assert_tar_has_no_escape_secret(&bytes, matches!(kind, ArchiveKind::TarGz))?;
        }
    }

    Ok(())
}

/// In-root symlinks may still be followed when `--no-symlinks` is off, so the
/// pointed-to file contents appear under the symlink's name.
#[rstest]
fn zip_follows_in_root_symlink_when_symlinks_allowed(
    #[with(&["--enable-zip"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let resp = reqwest_client
        .get(server.url().join(ArchiveKind::Zip.download_param())?)
        .send()?
        .error_for_status()?;
    let mut archive = ZipArchive::new(Cursor::new(resp.bytes()?))?;

    // fixtures create `file_symlink` -> `test.txt` with content "Test Hello Yes"
    let mut found_followed = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.name().ends_with("file_symlink") && entry.is_file() {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            assert_eq!(contents, "Test Hello Yes");
            found_followed = true;
        }
    }
    assert!(
        found_followed,
        "expected in-root file_symlink to be followed into the ZIP"
    );

    Ok(())
}

fn assert_tar_has_no_escape_secret(bytes: &[u8], gzipped: bool) -> Result<(), Error> {
    use std::io::Cursor;

    let plain: Vec<u8> = if gzipped {
        let mut decoder = libflate::gzip::Decoder::new(Cursor::new(bytes))?;
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;
        buf
    } else {
        bytes.to_vec()
    };

    let mut archive = tar::Archive::new(Cursor::new(plain));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        assert!(
            !path.to_string_lossy().contains(ESCAPE_LINK_NAME),
            "TAR must not contain escape symlink path '{}'",
            path.display()
        );
        if entry.header().entry_type().is_file() {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            assert!(
                !contents.contains("MINISERVE-OUTSIDE-SECRET"),
                "TAR entry '{}' leaked outside-root secret",
                path.display()
            );
        }
    }
    Ok(())
}

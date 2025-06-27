use std::process::Command;

use assert_cmd::prelude::*;
use assert_fs::TempDir;
use predicates::str::contains;
use reqwest::{Method, blocking::Client};
use reqwest_dav::{
    ClientBuilder as DavClientBuilder,
    list_cmd::{ListEntity, ListFile, ListFolder},
};
use rstest::rstest;

mod fixtures;

use crate::fixtures::{
    DIR_BEHIND_SYMLINKED_DIR, DIRECTORIES, DIRECTORY_SYMLINK, Error,
    FILE_IN_DIR_BEHIND_SYMLINKED_DIR, FILE_SYMLINK, FILES, HIDDEN_DIRECTORIES, HIDDEN_FILES,
    TestServer, server, tmpdir,
};

#[rstest]
#[case(server(&["--enable-webdav"]), true)]
#[case(server(&[] as &[&str]), false)]
fn webdav_flag_works(
    #[case] server: TestServer,
    #[case] should_respond: bool,
) -> Result<(), Error> {
    let client = Client::new();
    let response = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), server.url())
        .header("Depth", "1")
        .send()?;

    assert_eq!(should_respond, response.status().is_success());

    Ok(())
}

#[rstest]
fn webdav_advertised_in_options(
    #[with(&["--enable-webdav"])] server: TestServer,
) -> Result<(), Error> {
    let response = Client::new()
        .request(Method::OPTIONS, server.url())
        .send()?
        .error_for_status()?;

    let headers = response.headers();
    let allow = headers.get("allow").unwrap().to_str()?;

    assert!(allow.contains("OPTIONS") && allow.contains("PROPFIND"));
    assert!(headers.get("dav").is_some());

    Ok(())
}

fn list_webdav(url: url::Url, path: &str) -> Result<Vec<ListEntity>, reqwest_dav::Error> {
    let client = DavClientBuilder::new().set_host(url.to_string()).build()?;

    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async { client.list(path, reqwest_dav::Depth::Number(1)).await })
}

#[rstest]
#[case(server(&["--enable-webdav"]), false)]
#[case(server(&["--enable-webdav", "--hidden"]), true)]
fn webdav_respects_hidden_flag(
    #[case] server: TestServer,
    #[case] hidden_should_show: bool,
) -> Result<(), Error> {
    let list = list_webdav(server.url(), "/")?;

    assert_eq!(
        hidden_should_show,
        list.iter().any(|el|
            matches!(el, ListEntity::File(ListFile { href, .. }) if href.contains(HIDDEN_FILES[0]))
        )
    );

    assert_eq!(
        hidden_should_show,
        list.iter().any(|el|
            matches!(el, ListEntity::Folder(ListFolder { href, .. }) if href.contains(HIDDEN_DIRECTORIES[0]))
        )
    );

    Ok(())
}

#[rstest]
#[case(server(&["--enable-webdav"]), true)]
#[case(server(&["--enable-webdav", "--no-symlinks"]), false)]
fn webdav_respects_no_symlink_flag(#[case] server: TestServer, #[case] symlinks_should_show: bool) {
    let list = list_webdav(server.url(), "/").unwrap();

    assert_eq!(
        symlinks_should_show,
        list.iter().any(|el|
            matches!(el, ListEntity::File(ListFile { href, .. }) if href.contains(FILE_SYMLINK))
        ),
    );

    assert_eq!(
        symlinks_should_show,
        list.iter().any(|el|
            matches!(el, ListEntity::Folder(ListFolder { href, .. }) if href.contains(DIRECTORY_SYMLINK))
        ),
    );

    let list_linked = list_webdav(server.url(), &format!("/{DIRECTORY_SYMLINK}"));
    assert_eq!(symlinks_should_show, list_linked.is_ok());

    let list_nested_dir = list_webdav(server.url(), &format!("/{DIR_BEHIND_SYMLINKED_DIR}"));
    assert_eq!(symlinks_should_show, list_nested_dir.is_ok());

    let list_nested_file = list_webdav(
        server.url(),
        &format!("/{FILE_IN_DIR_BEHIND_SYMLINKED_DIR}"),
    );
    assert_eq!(symlinks_should_show, list_nested_file.is_ok());
}

#[rstest]
fn webdav_works_with_route_prefix(
    #[with(&["--enable-webdav", "--route-prefix", "test-prefix"])] server: TestServer,
) -> Result<(), Error> {
    let prefixed_list = list_webdav(server.url().join("test-prefix")?, "/")?;

    assert!(
        prefixed_list.iter().any(|el|
            matches!(el, ListEntity::Folder(ListFolder { href, .. }) if href.contains(DIRECTORIES[0]))
        )
    );

    let root_list = list_webdav(server.url(), "/");

    assert!(root_list.is_err());

    Ok(())
}

// timeout is used in case the binary does not exit as expected and starts waiting for requests
#[rstest]
#[timeout(std::time::Duration::from_secs(1))]
fn webdav_single_file_refuses_starting(tmpdir: TempDir) {
    Command::cargo_bin("miniserve")
        .unwrap()
        .current_dir(tmpdir.path())
        .arg(FILES[0])
        .arg("--enable-webdav")
        .assert()
        .failure()
        .stderr(contains(format!(
            "Error: The --enable-webdav option was provided, but the serve path '{}' is a file",
            FILES[0]
        )));
}

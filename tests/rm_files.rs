mod fixtures;

use anyhow::bail;
use assert_fs::fixture::TempDir;
use fixtures::{server, server_no_stderr, tmpdir, TestServer};
use percent_encoding::utf8_percent_encode;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use rstest::rstest;
use std::{
    iter,
    path::{Component, Path},
};
use url::Url;

use crate::fixtures::{
    DEEPLY_NESTED_FILE, DIRECTORIES, FILES, HIDDEN_DIRECTORIES, HIDDEN_FILES,
    NESTED_FILES_UNDER_SINGLE_ROOT,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/",
    "src/path_utils.rs"
));

/// Construct a path for a GET request,
/// with each path component being separately encoded.
fn make_get_path(unencoded_path: impl AsRef<Path>) -> String {
    unencoded_path
        .as_ref()
        .components()
        .map(|comp| match comp {
            Component::Prefix(_) | Component::RootDir => unreachable!("Not currently used"),
            Component::CurDir => ".",
            Component::ParentDir => "..",
            Component::Normal(comp) => comp.to_str().unwrap(),
        })
        .map(|comp| utf8_percent_encode(comp, percent_encode_sets::COMPONENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Construct a path for a deletion POST request without any further encoding.
///
/// This should be kept consistent with implementation.
fn make_del_path(unencoded_path: impl AsRef<Path>) -> String {
    format!("rm?path=/{}", make_get_path(unencoded_path))
}

fn assert_rm_ok(base_url: Url, unencoded_paths: &[impl AsRef<Path>]) -> anyhow::Result<()> {
    let client = Client::new();

    for file_path in unencoded_paths.iter().map(AsRef::as_ref) {
        // encode
        let get_url = base_url.join(&make_get_path(file_path))?;
        let del_url = base_url.join(&make_del_path(file_path))?;
        println!("===== {file_path:?} =====");
        println!("{get_url}, {del_url}");

        // check path exists
        let _get_res = client.get(get_url.clone()).send()?.error_for_status()?;

        // delete
        let _del_res = client.post(del_url).send()?.error_for_status()?;

        // check path is gone
        let get_res = client.get(get_url).send()?;
        if get_res.status() != StatusCode::NOT_FOUND {
            bail!("Unexpected status code: {}", get_res.status());
        }
    }

    Ok(())
}

/// The `check_paths_exist` parameter allows skipping this check before and after
/// the deletion attempt in case these paths should be inaccessible via GET.
fn assert_rm_err(
    base_url: Url,
    unencoded_paths: &[impl AsRef<Path>],
    check_paths_exist: bool,
) -> anyhow::Result<()> {
    let client = Client::new();

    for file_path in unencoded_paths.iter().map(AsRef::as_ref) {
        // encode
        let get_url = base_url.join(&make_get_path(file_path))?;
        let del_url = base_url.join(&make_del_path(file_path))?;
        println!("===== {file_path:?} =====");
        println!("{get_url}, {del_url}");

        // check path exists
        if check_paths_exist {
            let _get_res = client.get(get_url.clone()).send()?.error_for_status()?;
        }

        // delete
        let del_res = client.post(del_url).send()?;
        if !del_res.status().is_client_error() {
            bail!("Unexpected status code: {}", del_res.status());
        }

        // check path still exists
        if check_paths_exist {
            let _get_res = client.get(get_url).send()?.error_for_status()?;
        }
    }

    Ok(())
}

#[rstest]
fn rm_disabled_by_default(server: TestServer) -> anyhow::Result<()> {
    let paths = [FILES, DIRECTORIES]
        .concat()
        .into_iter()
        .map(Path::new)
        .chain(iter::once(DEEPLY_NESTED_FILE.as_ref()))
        .collect::<Vec<_>>();
    assert_rm_err(server.url(), &paths, true)
}

#[rstest]
fn rm_disabled_by_default_with_hidden(#[with(&["-H"])] server: TestServer) -> anyhow::Result<()> {
    let paths = [FILES, HIDDEN_FILES, DIRECTORIES, HIDDEN_DIRECTORIES]
        .concat()
        .into_iter()
        .map(Path::new)
        .chain(iter::once(DEEPLY_NESTED_FILE.as_ref()))
        .collect::<Vec<_>>();
    assert_rm_err(server.url(), &paths, true)
}

#[rstest]
fn rm_works(#[with(&["-R"])] server: TestServer) -> anyhow::Result<()> {
    let paths = [FILES, DIRECTORIES]
        .concat()
        .into_iter()
        .map(Path::new)
        .chain(iter::once(DEEPLY_NESTED_FILE.as_ref()))
        .collect::<Vec<_>>();
    assert_rm_ok(server.url(), &paths)
}

#[rstest]
fn cannot_rm_hidden_when_disallowed(
    #[with(&["-R"])] server_no_stderr: TestServer,
) -> anyhow::Result<()> {
    assert_rm_err(
        server_no_stderr.url(),
        &[HIDDEN_FILES, HIDDEN_DIRECTORIES].concat(),
        false,
    )
}

#[rstest]
fn can_rm_hidden_when_allowed(
    #[with(&["-R", "-H"])] server_no_stderr: TestServer,
) -> anyhow::Result<()> {
    assert_rm_ok(
        server_no_stderr.url(),
        &[HIDDEN_FILES, HIDDEN_DIRECTORIES].concat(),
    )
}

/// This test runs the server with --allowed-rm-dir argument and checks that
/// deletions in a different directory are actually prevented.
#[rstest]
#[case(server_no_stderr(&["-R", "someOtherDir"]))]
#[case(server_no_stderr(&["-R", "someDir/some_other_sub_dir"]))]
fn rm_is_restricted(#[case] server: TestServer) -> anyhow::Result<()> {
    assert_rm_err(server.url(), &NESTED_FILES_UNDER_SINGLE_ROOT, true)
}

/// This test runs the server with --allowed-rm-dir argument and checks that
/// deletions of the specified directories themselves are allowed.
///
/// Both ways of specifying multiple directories are tested.
#[rstest]
#[case(server(&["-R", "dira,dirb,dirc"]))]
#[case(server(&["-R", "dira", "-R", "dirb", "-R", "dirc"]))]
fn can_rm_allowed_dir(#[case] server: TestServer) -> anyhow::Result<()> {
    assert_rm_ok(server.url(), DIRECTORIES)
}

/// This tests that we can delete from directories specified by --allow-rm-dir.
#[rstest]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir/alpha")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir//alpha")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir/././alpha")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir/some_sub_dir")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir/some_sub_dir/")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir//some_sub_dir")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir/./some_sub_dir")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir/some_sub_dir/bravo")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir//some_sub_dir//bravo")]
#[case(server_no_stderr(&["-R", "someDir"]), "someDir/./some_sub_dir/../some_sub_dir/bravo")]
#[case(server_no_stderr(&["-R", "someDir/some_sub_dir"]), "someDir/some_sub_dir/bravo")]
#[case(server_no_stderr(&["-R", Path::new("someDir/some_sub_dir").to_str().unwrap()]),
    "someDir/some_sub_dir/bravo")]
fn can_rm_from_allowed_dir(#[case] server: TestServer, #[case] file: &str) -> anyhow::Result<()> {
    assert_rm_ok(server.url(), &[file])
}

/// Test deleting from symlinked directories that point to outside the server root.
#[rstest]
#[case(server(&["-R"]), true)]
#[case(server_no_stderr(&["-R", "--no-symlinks"]), false)]
fn rm_from_symlinked_dir(
    #[case] server: TestServer,
    #[case] should_succeed: bool,
    #[from(tmpdir)] target: TempDir,
) -> anyhow::Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::symlink as symlink_dir;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir;

    // create symlink
    let link: &Path = Path::new("linked");
    symlink_dir(target.path(), server.path().join(link))?;

    let files_through_link = [FILES, DIRECTORIES]
        .concat()
        .iter()
        .map(|name| link.join(name))
        .collect::<Vec<_>>();
    if should_succeed {
        assert_rm_ok(server.url(), &files_through_link)
    } else {
        assert_rm_err(server.url(), &files_through_link, false)
    }
}

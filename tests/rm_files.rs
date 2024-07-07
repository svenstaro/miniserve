mod fixtures;

use anyhow::bail;
use assert_fs::fixture::TempDir;
use fixtures::{server, server_no_stderr, tmpdir, TestServer};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use rstest::rstest;
use std::path::Path;
use url::Url;

use crate::fixtures::{
    DEEPLY_NESTED_FILE, DIRECTORIES, FILES, HIDDEN_DIRECTORIES, HIDDEN_FILES,
    NESTED_FILES_UNDER_SINGLE_ROOT,
};

fn assert_rm_ok(base_url: Url, paths: &[impl AsRef<str>]) -> anyhow::Result<()> {
    let client = Client::new();

    for path in paths.iter().map(AsRef::as_ref) {
        // check path exists
        let _get_res = client
            .get(base_url.join(path)?)
            .send()?
            .error_for_status()?;

        // delete
        let req_path = format!("rm?path=/{path}");
        let _del_res = client
            .post(base_url.join(&req_path)?)
            .send()?
            .error_for_status()?;

        // check path is gone
        let get_res = client.get(base_url.join(path)?).send()?;
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
    paths: &[impl AsRef<str>],
    check_paths_exist: bool,
) -> anyhow::Result<()> {
    let client = Client::new();

    for path in paths.iter().map(AsRef::as_ref) {
        // check path exists
        if check_paths_exist {
            let _get_res = client
                .get(base_url.join(path)?)
                .send()?
                .error_for_status()?;
        }

        // delete
        let req_path = format!("rm?path=/{path}");
        let del_res = client.post(base_url.join(&req_path)?).send()?;
        if !del_res.status().is_client_error() {
            bail!("Unexpected status code: {}", del_res.status());
        }

        // check path still exists
        if check_paths_exist {
            let _get_res = client
                .get(base_url.join(path)?)
                .send()?
                .error_for_status()?;
        }
    }

    Ok(())
}

#[rstest]
fn rm_disabled_by_default(server: TestServer) -> anyhow::Result<()> {
    assert_rm_err(
        server.url(),
        &[
            FILES,
            HIDDEN_FILES,
            DIRECTORIES,
            HIDDEN_DIRECTORIES,
            &[DEEPLY_NESTED_FILE],
        ]
        .concat(),
        true,
    )
}

#[rstest]
fn rm_works(#[with(&["-R"])] server: TestServer) -> anyhow::Result<()> {
    assert_rm_ok(
        server.url(),
        &[FILES, DIRECTORIES, &[DEEPLY_NESTED_FILE]].concat(),
    )
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
    assert_rm_err(server.url(), NESTED_FILES_UNDER_SINGLE_ROOT, true)
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
    const LINK_NAME: &str = "linked";
    symlink_dir(target.path(), server.path().join(LINK_NAME))?;

    let files_through_link = &[FILES, DIRECTORIES]
        .concat()
        .iter()
        .map(|name| format!("{LINK_NAME}/{name}"))
        .collect::<Vec<_>>();
    if should_succeed {
        assert_rm_ok(server.url(), files_through_link)
    } else {
        assert_rm_err(server.url(), files_through_link, false)
    }
}

use std::collections::HashMap;

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{StatusCode, blocking::Client};
use rstest::rstest;

mod fixtures;

use crate::fixtures::{DIRECTORIES, Error, TestServer, server};

/// Test that we can get dir size for plain paths as well as percent-encoded paths
#[rstest]
#[case(DIRECTORIES[0].to_string())]
#[case(DIRECTORIES[1].to_string())]
#[case(DIRECTORIES[2].to_string())]
#[case(utf8_percent_encode(DIRECTORIES[0], NON_ALPHANUMERIC).to_string())]
#[case(utf8_percent_encode(DIRECTORIES[1], NON_ALPHANUMERIC).to_string())]
#[case(utf8_percent_encode(DIRECTORIES[2], NON_ALPHANUMERIC).to_string())]
fn api_dir_size(
    #[case] dir: String,
    #[with(&["--directory-size"])] server: TestServer,
) -> Result<(), Error> {
    let mut command = HashMap::new();
    command.insert("DirSize", dir);

    let resp = Client::new()
        .post(server.url().join("__miniserve_internal/api")?)
        .json(&command)
        .send()?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_ne!(resp.text()?, "0 B");

    Ok(())
}

/// Test for path traversal vulnerability (CWE-22) in DirSize parameter.
#[rstest]
#[case("/tmp")] // Not CWE-22, but `foo` isn't a directory
#[case("/../foo")]
#[case("../foo")]
#[case("../tmp")]
#[case("/tmp")]
#[case("/foo")]
#[case("C:/foo")]
#[case(r"C:\foo")]
#[case(r"\foo")]
fn api_dir_size_prevent_path_transversal_attacks(
    #[with(&["--directory-size"])] server: TestServer,
    #[case] path: &str,
) -> Result<(), Error> {
    let mut command = HashMap::new();
    command.insert("DirSize", path);

    let resp = Client::new()
        .post(server.url().join("__miniserve_internal/api")?)
        .json(&command)
        .send()?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

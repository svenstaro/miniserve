use std::collections::HashMap;

use reqwest::{StatusCode, blocking::Client};
use rstest::rstest;

mod fixtures;

use crate::fixtures::{DIRECTORIES, Error, TestServer, server};

#[rstest]
fn api_dir_size(server: TestServer) -> Result<(), Error> {
    let mut command = HashMap::new();
    command.insert("DirSize", DIRECTORIES[0]);

    let resp = Client::new()
        .post(server.url().join(&format!("__miniserve_internal/api"))?)
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
    server: TestServer,
    #[case] path: &str,
) -> Result<(), Error> {
    let mut command = HashMap::new();
    command.insert("DirSize", path);

    let resp = Client::new()
        .post(server.url().join(&format!("__miniserve_internal/api"))?)
        .json(&command)
        .send()?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

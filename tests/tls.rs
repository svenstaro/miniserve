use assert_cmd::Command;
use predicates::str::contains;
use reqwest::blocking::ClientBuilder;
use rstest::rstest;
use select::{document::Document, node::Node};

mod fixtures;

use crate::fixtures::{Error, FILES, TestServer, server};

/// Can start the server with TLS and receive encrypted responses.
#[rstest]
#[case(server(&[
        "--tls-cert", "tests/data/cert_rsa.pem",
        "--tls-key", "tests/data/key_pkcs8.pem",
]))]
#[case(server(&[
        "--tls-cert", "tests/data/cert_rsa.pem",
        "--tls-key", "tests/data/key_pkcs1.pem",
]))]
#[case(server(&[
        "--tls-cert", "tests/data/cert_ec.pem",
        "--tls-key", "tests/data/key_ec.pem",
]))]
fn tls_works(#[case] server: TestServer) -> Result<(), Error> {
    let client = ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .build()?;
    let body = client.get(server.url()).send()?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
    }

    Ok(())
}

/// Wrong path for cert throws error.
#[rstest]
fn wrong_path_cert() -> Result<(), Error> {
    Command::cargo_bin("miniserve")?
        .args(["--tls-cert", "wrong", "--tls-key", "tests/data/key.pem"])
        .assert()
        .failure()
        .stderr(contains("Error: Couldn't access TLS certificate \"wrong\""));

    Ok(())
}

/// Wrong paths for key throws errors.
#[rstest]
fn wrong_path_key() -> Result<(), Error> {
    Command::cargo_bin("miniserve")?
        .args(["--tls-cert", "tests/data/cert.pem", "--tls-key", "wrong"])
        .assert()
        .failure()
        .stderr(contains("Error: Couldn't access TLS key \"wrong\""));

    Ok(())
}

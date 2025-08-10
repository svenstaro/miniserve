use reqwest::{StatusCode, blocking::Client};
use rstest::rstest;
use select::{document::Document, predicate::Text};

mod fixtures;

use crate::fixtures::{Error, FILES, TestServer, server};

#[rstest]
#[case("joe", "123")]
#[case("bob", "123")]
#[case("bill", "")]
fn auth_file_accepts(
    #[with(&["--auth-file", "tests/data/auth1.txt"])] server: TestServer,
    #[case] client_username: &str,
    #[case] client_password: &str,
) -> Result<(), Error> {
    let client = Client::new();
    let response = client
        .get(server.url())
        .basic_auth(client_username, Some(client_password))
        .send()?;

    let status_code = response.status();
    assert_eq!(status_code, StatusCode::OK);

    let body = response.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(Text).any(|x| x.text() == file));
    }

    Ok(())
}

#[rstest]
#[case("joe", "wrongpassword")]
#[case("bob", "")]
#[case("nonexistentuser", "wrongpassword")]
fn auth_file_rejects(
    #[with(&["--auth-file", "tests/data/auth1.txt"])] server: TestServer,
    #[case] client_username: &str,
    #[case] client_password: &str,
) -> Result<(), Error> {
    let client = Client::new();
    let status = client
        .get(server.url())
        .basic_auth(client_username, Some(client_password))
        .send()?
        .status();

    assert_eq!(status, StatusCode::UNAUTHORIZED);

    Ok(())
}

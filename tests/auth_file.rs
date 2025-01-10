use reqwest::{blocking::Client, StatusCode};
use rstest::rstest;
use select::{document::Document, predicate::Text};

mod fixtures;

use crate::fixtures::{server, server_no_stderr, Error, FILES};

#[rstest(
    cli_auth_file_arg,
    client_username,
    client_password,
    case("tests/data/auth1.txt", "joe", "123"),
    case("tests/data/auth1.txt", "bob", "123"),
    case("tests/data/auth1.txt", "bill", "")
)]
fn auth_file_accepts(
    cli_auth_file_arg: &str,
    client_username: &str,
    client_password: &str,
) -> Result<(), Error> {
    let server = server(&["--auth-file", cli_auth_file_arg]);
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

#[rstest(
    cli_auth_file_arg,
    client_username,
    client_password,
    case("tests/data/auth1.txt", "joe", "wrongpassword"),
    case("tests/data/auth1.txt", "bob", ""),
    case("tests/data/auth1.txt", "nonexistentuser", "wrongpassword")
)]
fn auth_file_rejects(
    cli_auth_file_arg: &str,
    client_username: &str,
    client_password: &str,
) -> Result<(), Error> {
    let server = server_no_stderr(&["--auth-file", cli_auth_file_arg]);
    let client = Client::new();
    let status = client
        .get(server.url())
        .basic_auth(client_username, Some(client_password))
        .send()?
        .status();

    assert_eq!(status, StatusCode::UNAUTHORIZED);

    Ok(())
}

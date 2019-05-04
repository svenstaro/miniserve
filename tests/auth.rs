mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error, FILES};
use rstest::rstest_parametrize;
use select::document::Document;
use select::predicate::Text;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest_parametrize(
    cli_auth_arg, client_username, client_password,
    case("testuser:testpassword", "testuser", "testpassword"),
    case(
        "testuser:sha256:9f735e0df9a1ddc702bf0a1a7b83033f9f7153a00c29de82cedadc9957289b05",
        "testuser",
        "testpassword"
    ),
    case(
        "testuser:sha512:e9e633097ab9ceb3e48ec3f70ee2beba41d05d5420efee5da85f97d97005727587fda33ef4ff2322088f4c79e8133cc9cd9f3512f4d3a303cbdb5bc585415a00",
        "testuser",
        "testpassword"
    ),
)]
fn auth_works(
    tmpdir: TempDir,
    port: u16,
    cli_auth_arg: &str,
    client_username: &str,
    client_password: &str,
) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .arg("-a")
        .arg(cli_auth_arg)
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://localhost:{}", port).as_str())
        .basic_auth(client_username, Some(client_password))
        .send()?;

    let status_code = response.status();
    assert_eq!(status_code.canonical_reason(), Some("OK"));

    let body = response.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(Text).any(|x| x.text() == file));
    }

    child.kill()?;

    Ok(())
}

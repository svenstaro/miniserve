mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use rstest::rstest;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest(header,
    case("x-info: 123".to_string()),
    case("x-info1: 123\r\nx-info2: 345".to_string())
)]
fn custom_header_set(tmpdir: TempDir, port: u16, header: String) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .arg("--header")
        .arg(header.clone())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let resp = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?;

    let mut headers = [httparse::EMPTY_HEADER; 4];
    let mut header = header.clone();
    header.push('\n');
    httparse::parse_headers(header.as_bytes(), &mut headers)?;

    for h in headers.iter() {
        if h.name != httparse::EMPTY_HEADER.name {
            assert_eq!(resp.headers().get(h.name).unwrap(), h.value);
        }
    }

    child.kill()?;

    Ok(())
}

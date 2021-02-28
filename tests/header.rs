mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use rstest::rstest;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest(headers,
    case(vec!["x-info: 123".to_string()]),
    case(vec!["x-info1: 123".to_string(), "x-info2: 345".to_string()])
)]
fn custom_header_set(tmpdir: TempDir, port: u16, headers: Vec<String>) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .args(headers.iter().flat_map(|h| vec!["--header", h]))
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let resp = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?;

    for header in headers {
        let mut header_split = header.splitn(2, ':');
        let header_name = header_split.next().unwrap();
        let header_value = header_split.next().unwrap().trim();
        assert_eq!(resp.headers().get(header_name).unwrap(), header_value);
    }

    child.kill()?;

    Ok(())
}

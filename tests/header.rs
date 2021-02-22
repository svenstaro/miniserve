mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use rstest::rstest;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest]
fn custom_header_set(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let header_name = "x-info";
    let header_value = "123";
    let header_str = format!("{}: {}", header_name, header_value);

    let _ = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .arg("--header")
        .arg(header_str)
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let resp = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?;

    assert_eq!(resp.headers().get(header_name).unwrap(), header_value);

    Ok(())
}

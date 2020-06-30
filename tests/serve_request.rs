mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error, DIRECTORIES, FILES};
use rstest::rstest;
use select::document::Document;
use select::node::Node;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use regex::Regex;

#[rstest]
fn serves_requests_with_no_options(tmpdir: TempDir) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::blocking::get("http://localhost:8080")?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
    }

    child.kill()?;

    Ok(())
}

#[rstest]
fn serves_requests_with_non_default_port(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
    }
    for &directory in DIRECTORIES {
        assert!(parsed
            .find(|x: &Node| x.text() == directory)
            .next()
            .is_some());
        let dir_body =
            reqwest::blocking::get(format!("http://localhost:{}/{}", port, directory).as_str())?
                .error_for_status()?;
        let dir_body_parsed = Document::from_read(dir_body)?;
        for &file in FILES {
            assert!(dir_body_parsed
                .find(|x: &Node| x.text() == file)
                .next()
                .is_some());
        }
    }

    child.kill()?;

    Ok(())
}

#[rstest]
fn serves_requests_with_randomly_assigned_port(tmpdir: TempDir) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg("0".to_string())
        .stdout(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));
    child.kill()?;

    let output = child.wait_with_output().expect("Failed to read stdout");
    let all_text = String::from_utf8(output.stdout)?;

    let re = Regex::new(r"http://127.0.0.1:(\d+)").unwrap();
    let caps = re.captures(all_text.as_str()).unwrap();
    let port_num = caps.get(1).unwrap().as_str().parse::<u16>().unwrap();

    assert!(port_num > 0);

    Ok(())
}

#[rstest]
fn serves_requests_custom_index_notice(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("--index=not.html")
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));

    child.kill()?;
    let output = child.wait_with_output().expect("Failed to read stdout");
    let all_text = String::from_utf8(output.stdout);

    assert!(all_text
        .unwrap()
        .contains("The provided index file could not be found"));

    Ok(())
}

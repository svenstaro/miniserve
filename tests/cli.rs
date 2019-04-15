use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use assert_fs::prelude::*;
use clap::{crate_name, crate_version};
use portpicker::pick_unused_port;
use reqwest;
use select::document::Document;
use select::predicate::Text;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

type Error = Box<std::error::Error>;

static FILES: &[&str] = &["test.txt", "test.html", "test.mkv"];

// Test fixture which creates a temporary directory with a few files inside.
pub fn tmpdir() -> Result<TempDir, Error> {
    let tmpdir = assert_fs::TempDir::new()?;
    for &file in FILES {
        tmpdir.child(file).touch()?;
    }
    Ok(tmpdir)
}

#[test]
/// Starts and serves requests without any options.
fn starts_ok_with_no_option() -> Result<(), Error> {
    let tmpdir = tmpdir()?;
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::get("http://localhost:8080")?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(Text).any(|x| x.text() == file));
    }

    child.kill()?;

    Ok(())
}

#[test]
/// Starts and serves requests on a non-default port.
fn starts_ok_with_non_default_port() -> Result<(), Error> {
    let tmpdir = tmpdir()?;

    let port = pick_unused_port().unwrap();
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::get(format!("http://localhost:{}", port).as_str())?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(Text).any(|x| x.text() == file));
    }

    child.kill()?;

    Ok(())
}

#[test]
/// Show help and exit.
fn help_shows() -> Result<(), Error> {
    Command::cargo_bin("miniserve")?
        .arg("-h")
        .assert()
        .success();

    Ok(())
}

#[test]
/// Show version and exit.
fn version_shows() -> Result<(), Error> {
    Command::cargo_bin("miniserve")?
        .arg("-V")
        .assert()
        .success()
        .stdout(format!("{} {}\n", crate_name!(), crate_version!()));

    Ok(())
}

mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, server, tmpdir, Error, TestServer};
use regex::Regex;
use rstest::rstest;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest]
#[case(&["-i", "12.123.234.12"])]
#[case(&["-i", "::", "-i", "12.123.234.12"])]
fn bind_fails(tmpdir: TempDir, port: u16, #[case] args: &[&str]) -> Result<(), Error> {
    Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .args(args)
        .assert()
        .stderr(predicates::str::contains("Failed to bind server to"))
        .failure();

    Ok(())
}

#[rstest]
#[case(server(&[] as &[&str]), true, true)]
#[case(server(&["-i", "::"]), false, true)]
#[case(server(&["-i", "0.0.0.0"]), true, false)]
#[case(server(&["-i", "::", "-i", "0.0.0.0"]), true, true)]
fn bind_ipv4_ipv6(
    #[case] server: TestServer,
    #[case] bind_ipv4: bool,
    #[case] bind_ipv6: bool,
) -> Result<(), Error> {
    assert_eq!(
        reqwest::blocking::get(format!("http://127.0.0.1:{}", server.port()).as_str()).is_ok(),
        bind_ipv4
    );
    assert_eq!(
        reqwest::blocking::get(format!("http://[::1]:{}", server.port()).as_str()).is_ok(),
        bind_ipv6
    );

    Ok(())
}

#[rstest]
#[case(&[] as &[&str])]
#[case(&["-i", "::"])]
#[case(&["-i", "127.0.0.1"])]
#[case(&["-i", "0.0.0.0"])]
#[case(&["-i", "::", "-i", "0.0.0.0"])]
#[case(&["--random-route"])]
fn validate_printed_urls(tmpdir: TempDir, port: u16, #[case] args: &[&str]) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let urls_line = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|line| line.expect("Error reading stdout"))
        .filter(|line| line.starts_with("Serving path"))
        .next()
        .expect("no url printed to stdout");

    let urls = Regex::new(r"http://[a-zA-Z0-9\.\[\]:/]+")
        .unwrap()
        .captures_iter(urls_line.as_str())
        .map(|caps| caps.get(0).unwrap().as_str())
        .collect::<Vec<_>>();

    assert!(!urls.is_empty());

    for url in urls {
        reqwest::blocking::get(url)?.error_for_status()?;
    }

    child.kill()?;

    Ok(())
}

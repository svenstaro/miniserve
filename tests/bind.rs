mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, server, tmpdir, Error, TestServer};
use regex::Regex;
use rstest::rstest;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

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
        .stderr(predicates::str::contains("Failed to bind tcp"))
        .failure();

    Ok(())
}

#[cfg(unix)]
#[rstest]
#[case(&[] as &[&str])]
fn bind_uds_fails(tmpdir: TempDir, _port: u16, #[case] args: &[&str]) -> Result<(), Error> {
    // Make normal file at socket path
    let mut socket_path = tmpdir.path().to_path_buf();
    socket_path.push("socket");
    std::fs::File::create(&socket_path)?;

    Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-i")
        .arg(socket_path)
        .args(args)
        .assert()
        .stderr(predicates::str::contains(
            "unix socket path already exists and is not a unix socket",
        ))
        .failure();

    Ok(())
}

#[rstest]
#[case(server(&[] as &[&str]), true, true, false)]
#[case(server(&["-i", "::"]), false, true, false)]
#[case(server(&["-i", "0.0.0.0"]), true, false, false)]
#[case(server(&["-i", "::", "-i", "0.0.0.0"]), true, true, false)]
#[case(server(&["-i", "./miniserve.socket"]), false, false, true)]
#[case(server(&["-i", "::", "-i", "./miniserve.socket"]), false, true, true)]
#[case(server(&["-i", "0.0.0.0", "-i", "./miniserve.socket"]), true, false, true)]
#[case(server(&["-i", "::", "-i", "0.0.0.0", "-i", "./miniserve.socket"]), true, true, true)]
fn bind_ipv4_ipv6(
    #[case] server: TestServer,
    #[case] bind_ipv4: bool,
    #[case] bind_ipv6: bool,
    #[case] bind_unix: bool,
) -> Result<(), Error> {
    assert_eq!(
        reqwest::blocking::get(format!("http://127.0.0.1:{}", server.port()).as_str()).is_ok(),
        bind_ipv4
    );
    assert_eq!(
        reqwest::blocking::get(format!("http://[::1]:{}", server.port()).as_str()).is_ok(),
        bind_ipv6
    );

    #[cfg(unix)]
    assert_eq!(
        {
            let mut socket_path = server.path().to_path_buf();
            socket_path.push("miniserve.socket");

            let mut easy = curl::easy::Easy::new();
            easy.unix_socket_path(Some(socket_path.as_path())).unwrap();
            easy.url("localhost").unwrap();
            easy.write_function(|data| Ok(data.len())).unwrap();
            easy.perform().is_ok()
        },
        bind_unix
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
#[case(&["--route-prefix", "/prefix"])]
fn validate_printed_urls(tmpdir: TempDir, port: u16, #[case] args: &[&str]) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?;

    // WARN assumes urls list is terminated by an empty line
    let url_lines = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|line| line.expect("Error reading stdout"))
        .take_while(|line| !line.is_empty()) /* non-empty lines */
        .collect::<Vec<_>>();
    let url_lines = url_lines.join("\n");

    let urls = Regex::new(r"http://[a-zA-Z0-9\.\[\]:/]+")
        .unwrap()
        .captures_iter(url_lines.as_str())
        .map(|caps| caps.get(0).unwrap().as_str())
        .collect::<Vec<_>>();

    assert!(!urls.is_empty());

    for url in urls {
        reqwest::blocking::get(url)?.error_for_status()?;
    }

    child.kill()?;

    Ok(())
}

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use assert_cmd::{cargo, prelude::*};
use assert_fs::fixture::TempDir;
use regex::Regex;
use reqwest::blocking::Client;
use rstest::rstest;

mod fixtures;

#[cfg(not(windows))]
use crate::fixtures::run_in_faketty_kill_and_get_stdout;
use crate::fixtures::{Error, TestServer, port, reqwest_client, server, tmpdir};

/// The default port miniserve uses when `--port` is not given (see `CliArgs::port`).
#[cfg(not(windows))]
const DEFAULT_PORT: u16 = 8080;

#[rstest]
#[case(&["-i", "12.123.234.12"])]
#[case(&["-i", "::", "-i", "12.123.234.12"])]
fn bind_fails(tmpdir: TempDir, port: u16, #[case] args: &[&str]) -> Result<(), Error> {
    Command::new(cargo::cargo_bin!("miniserve"))
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
    reqwest_client: Client,
    #[case] bind_ipv4: bool,
    #[case] bind_ipv6: bool,
) -> Result<(), Error> {
    assert_eq!(
        reqwest_client
            .get(format!("http://127.0.0.1:{}", server.port()).as_str())
            .send()
            .is_ok(),
        bind_ipv4
    );
    assert_eq!(
        reqwest_client
            .get(format!("http://[::1]:{}", server.port()).as_str())
            .send()
            .is_ok(),
        bind_ipv6
    );

    Ok(())
}

/// When no explicit port is given and the default port is already taken, miniserve should fall
/// back to a random free port (only when a terminal is attached) instead of failing to start.
// Disabled for Windows because `fake_tty` does not currently support it.
#[rstest]
#[cfg(not(windows))]
fn falls_back_to_free_port_when_default_is_taken_in_tty(tmpdir: TempDir) -> Result<(), Error> {
    use std::net::TcpListener;

    // Make sure the default port is unavailable. Best-effort: if something else already holds it,
    // that is fine too, since the point is simply that the port cannot be bound.
    let _default_port_guard = TcpListener::bind(("127.0.0.1", DEFAULT_PORT));

    // Run without `--port` and inside a faked TTY so the interactive fallback kicks in.
    let mut template = Command::new(cargo::cargo_bin!("miniserve"));
    template.arg(tmpdir.path());
    let output = run_in_faketty_kill_and_get_stdout(&template)?;

    // The server must have started and advertised at least one URL ...
    assert!(
        output.contains("http://"),
        "miniserve did not advertise any URL; output was:\n{output}"
    );
    // ... but none of them may point at the occupied default port.
    assert!(
        !output.contains(&format!(":{DEFAULT_PORT}")),
        "miniserve advertised the occupied default port {DEFAULT_PORT}; output was:\n{output}"
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
fn validate_printed_urls(
    reqwest_client: Client,
    tmpdir: TempDir,
    port: u16,
    #[case] args: &[&str],
) -> Result<(), Error> {
    let mut child = Command::new(cargo::cargo_bin!("miniserve"))
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
        reqwest_client.get(url).send()?.error_for_status()?;
    }

    child.kill()?;

    Ok(())
}

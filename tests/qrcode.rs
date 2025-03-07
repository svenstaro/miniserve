use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use assert_cmd::prelude::CommandCargoExt;
use assert_fs::TempDir;
use rstest::rstest;
use select::{document::Document, predicate::Attr};

mod fixtures;

use crate::fixtures::{Error, TestServer, port, server, tmpdir};

#[rstest]
fn webpage_hides_qrcode_when_disabled(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Attr("id", "qrcode")).next().is_none());

    Ok(())
}

#[rstest]
fn webpage_shows_qrcode_when_enabled(#[with(&["-q"])] server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    let qr_container = parsed
        .find(Attr("id", "qrcode"))
        .next()
        .ok_or("QR container not found")?;
    let tooltip = qr_container
        .attr("title")
        .ok_or("QR container has no title")?;
    assert_eq!(tooltip, server.url().as_str());

    Ok(())
}

#[cfg(not(windows))]
fn run_in_faketty_kill_and_get_stdout(template: &Command) -> Result<String, Error> {
    use fake_tty::{bash_command, get_stdout};

    let cmd = {
        let bin = template.get_program().to_str().expect("not UTF8");
        let args = template
            .get_args()
            .map(|s| s.to_str().expect("not UTF8"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{bin} {args}")
    };
    let mut child = bash_command(&cmd)?.stdin(Stdio::null()).spawn()?;

    sleep(Duration::from_secs(1));

    child.kill()?;
    let output = child.wait_with_output().expect("Failed to read stdout");
    let all_text = get_stdout(output.stdout)?;

    Ok(all_text)
}

#[rstest]
// Disabled for Windows because `fake_tty` does not currently support it.
#[cfg(not(windows))]
fn qrcode_hidden_in_tty_when_disabled(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut template = Command::cargo_bin("miniserve")?;
    template.arg("-p").arg(port.to_string()).arg(tmpdir.path());

    let output = run_in_faketty_kill_and_get_stdout(&template)?;

    assert!(!output.contains("QR code for "));
    Ok(())
}

#[rstest]
// Disabled for Windows because `fake_tty` does not currently support it.
#[cfg(not(windows))]
fn qrcode_shown_in_tty_when_enabled(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut template = Command::cargo_bin("miniserve")?;
    template
        .arg("-p")
        .arg(port.to_string())
        .arg("-q")
        .arg(tmpdir.path());

    let output = run_in_faketty_kill_and_get_stdout(&template)?;

    assert!(output.contains("QR code for "));
    Ok(())
}

#[rstest]
fn qrcode_hidden_in_non_tty_when_enabled(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg("-q")
        .arg(tmpdir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));

    child.kill()?;
    let output = child.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8(output.stdout)?;

    assert!(!stdout.contains("QR code for "));
    Ok(())
}

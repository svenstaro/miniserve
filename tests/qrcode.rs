mod fixtures;

use assert_cmd::prelude::CommandCargoExt;
use assert_fs::TempDir;
use fixtures::{port, server_no_stderr, tmpdir, Error, TestServer};
use reqwest::StatusCode;
use rstest::rstest;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

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
        format!("{} {}", bin, args)
    };
    let mut child = bash_command(&cmd).spawn()?;

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
        .spawn()?;

    sleep(Duration::from_secs(1));

    child.kill()?;
    let output = child.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8(output.stdout)?;

    assert!(!stdout.contains("QR code for "));
    Ok(())
}

#[rstest]
fn get_svg_qrcode(#[from(server_no_stderr)] server: TestServer) -> Result<(), Error> {
    // Ok
    let resp = reqwest::blocking::get(server.url().join("?qrcode=test")?)?;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text()?;
    assert!(body.contains("qr_code_page"));
    assert!(body.contains("<svg"));

    // Err
    let content: String = "0".repeat(8192);
    let resp = reqwest::blocking::get(server.url().join(&format!("?qrcode={}", content))?)?;

    assert_eq!(resp.status(), StatusCode::URI_TOO_LONG);

    Ok(())
}

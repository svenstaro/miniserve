mod fixtures;

use assert_cmd::prelude::*;
use clap::{crate_name, crate_version};
use clap_generate::Shell;
use fixtures::Error;
use std::process::Command;

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

#[test]
/// Print completions and exit.
fn print_completions() -> Result<(), Error> {
    for shell in Shell::arg_values() {
        Command::cargo_bin("miniserve")?
            .arg("--print-completions")
            .arg(shell.get_name())
            .assert()
            .success();
    }

    Ok(())
}

#[test]
/// Print completions rejects invalid shells.
fn print_completions_invalid_shell() -> Result<(), Error> {
    Command::cargo_bin("miniserve")?
        .arg("--print-completions")
        .arg("fakeshell")
        .assert()
        .failure();

    Ok(())
}

use std::process::Command;

use assert_cmd::prelude::*;
use clap::{ValueEnum, crate_name, crate_version};
use clap_complete::Shell;

mod fixtures;

use crate::fixtures::Error;

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
    for shell in Shell::value_variants() {
        Command::cargo_bin("miniserve")?
            .arg("--print-completions")
            .arg(shell.to_string())
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

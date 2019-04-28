pub use assert_cmd::prelude::*;
pub use assert_fs::fixture::TempDir;
pub use assert_fs::prelude::*;
pub use clap::{crate_name, crate_version};
pub use port_check::free_local_port;
pub use reqwest;
pub use reqwest::multipart;
pub use rstest::rstest;
pub use select::document::Document;
pub use select::predicate::{Attr, Text};
pub use std::process::{Command, Stdio};
pub use std::thread::sleep;
pub use std::time::Duration;
pub use rstest::rstest_parametrize;

/// Error type used by tests
pub type Error = Box<std::error::Error>;

/// File names for testing purpose
pub static FILES: &[&str] = &["test.txt", "test.html", "test.mkv"];

/// Test fixture which creates a temporary directory with a few files inside.
pub fn tmpdir() -> TempDir {
    let tmpdir = assert_fs::TempDir::new().expect("Couldn't create a temp dir for tests");
    for &file in FILES {
        tmpdir
            .child(file)
            .write_str("Test Hello Yes")
            .expect("Couldn't write to file");
    }
    tmpdir
}

/// Get a free port.
pub fn port() -> u16 {
    free_local_port().expect("Couldn't find a free local port")
}

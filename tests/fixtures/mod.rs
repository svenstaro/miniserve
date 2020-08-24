use assert_fs::fixture::TempDir;
use assert_fs::prelude::*;
use port_check::free_local_port;
use rstest::fixture;

/// Error type used by tests
pub type Error = Box<dyn std::error::Error>;

/// File names for testing purpose
#[allow(dead_code)]
pub static FILES: &[&str] = &["test.txt", "test.html", "test.mkv", "test \" \' & < >.csv"];

/// Directory names for testing purpose
#[allow(dead_code)]
pub static DIRECTORIES: &[&str] = &["dira/", "dirb/", "dirc/"];

/// Name of a deeply nested file
#[allow(dead_code)]
pub static DEEPLY_NESTED_FILE: &str = "very/deeply/nested/test.rs";

/// Test fixture which creates a temporary directory with a few files and directories inside.
/// The directories also contain files.
#[fixture]
#[allow(dead_code)]
pub fn tmpdir() -> TempDir {
    let tmpdir = assert_fs::TempDir::new().expect("Couldn't create a temp dir for tests");
    for &file in FILES {
        tmpdir
            .child(file)
            .write_str("Test Hello Yes")
            .expect("Couldn't write to file");
    }
    for &directory in DIRECTORIES {
        for &file in FILES {
            tmpdir
                .child(format!("{}{}", directory, file))
                .write_str(&format!("This is {}{}", directory, file))
                .expect("Couldn't write to file");
        }
    }
    tmpdir
        .child(&DEEPLY_NESTED_FILE)
        .write_str("File in a deeply nested directory.")
        .expect("Couldn't write to file");
    tmpdir
}

/// Get a free port.
#[fixture]
#[allow(dead_code)]
pub fn port() -> u16 {
    free_local_port().expect("Couldn't find a free local port")
}

use assert_fs::fixture::TempDir;
use assert_fs::prelude::*;
use port_check::free_local_port;
use rstest::fixture;

/// Error type used by tests
pub type Error = Box<std::error::Error>;

/// File names for testing purpose
#[allow(dead_code)]
pub static FILES: &[&str] = &["test.txt", "test.html", "test.mkv"];

/// Test fixture which creates a temporary directory with a few files inside.
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
    tmpdir
}

/// Get a free port.
#[fixture]
#[allow(dead_code)]
pub fn port() -> u16 {
    free_local_port().expect("Couldn't find a free local port")
}

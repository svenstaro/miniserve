use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use assert_fs::prelude::*;
use port_check::free_local_port;
use reqwest::Url;
use rstest::fixture;

/// Error type used by tests
pub type Error = Box<dyn std::error::Error>;

/// File names for testing purpose
pub static FILES: &[&str] = &[
    "test.txt",
    "test.html",
    "test.mkv",
    #[cfg(not(windows))]
    "test \" \' & < >.csv",
    #[cfg(not(windows))]
    "new\nline",
    "😀.data",
    "⎙.mp4",
    "#[]{}()@!$&'`+,;= %20.test",
    #[cfg(unix)]
    ":?#[]{}<>()@!$&'`|*+,;= %20.test",
    #[cfg(not(windows))]
    "foo\\bar.test",
];

/// Hidden files for testing purpose
pub static HIDDEN_FILES: &[&str] = &[".hidden_file1", ".hidden_file2"];

/// Directory names for testing purpose
pub static DIRECTORIES: &[&str] = &["dira/", "dirb/", "dir space/"];

/// Hidden directories for testing purpose
pub static HIDDEN_DIRECTORIES: &[&str] = &[".hidden_dir1/", ".hidden space dir/"];

/// Name of a deeply nested file
pub static DEEPLY_NESTED_FILE: &str = "very/deeply/nested/test.rs";

/// Name of a symlink pointing to a directory
pub static DIRECTORY_SYMLINK: &str = "dir_symlink/";

/// Name of a directory inside a symlinked directory
#[allow(unused)]
pub static DIR_BEHIND_SYMLINKED_DIR: &str = "dir_symlink/nested";

/// Name of a file inside a directory inside a symlinked directory
pub static FILE_IN_DIR_BEHIND_SYMLINKED_DIR: &str = "dir_symlink/nested/file";

/// Name of a symlink pointing to a file
pub static FILE_SYMLINK: &str = "file_symlink";

/// Name of a symlink pointing to a path that doesn't exist
pub static BROKEN_SYMLINK: &str = "broken_symlink";

/// Test fixture which creates a temporary directory with a few files and directories inside.
/// The directories also contain files.
#[fixture]
pub fn tmpdir() -> TempDir {
    let tmpdir = assert_fs::TempDir::new().expect("Couldn't create a temp dir for tests");
    let mut files = FILES.to_vec();
    files.extend_from_slice(HIDDEN_FILES);
    for file in &files {
        tmpdir
            .child(file)
            .write_str("Test Hello Yes")
            .expect("Couldn't write to file");
    }

    let mut directories = DIRECTORIES.to_vec();
    directories.extend_from_slice(HIDDEN_DIRECTORIES);
    for directory in directories {
        for file in &files {
            tmpdir
                .child(format!("{directory}{file}"))
                .write_str(&format!("This is {directory}{file}"))
                .expect("Couldn't write to file");
        }
    }

    tmpdir
        .child(DEEPLY_NESTED_FILE)
        .write_str("File in a deeply nested directory.")
        .expect("Couldn't write to file");

    tmpdir
        .child(DIRECTORY_SYMLINK.strip_suffix("/").unwrap())
        .symlink_to_dir(DIRECTORIES[0].strip_suffix("/").unwrap())
        .expect("Couldn't create symlink to dir");

    tmpdir
        .child(FILE_SYMLINK)
        .symlink_to_file(FILES[0])
        .expect("Couldn't create symlink to file");

    tmpdir
        .child(BROKEN_SYMLINK)
        .symlink_to_file("broken_symlink")
        .expect("Couldn't create broken symlink");

    tmpdir
        .child(FILE_IN_DIR_BEHIND_SYMLINKED_DIR)
        .write_str("something")
        .expect("Couldn't write symlink nexted file");

    tmpdir
}

/// Get a free port.
#[fixture]
pub fn port() -> u16 {
    free_local_port().expect("Couldn't find a free local port")
}

/// Run miniserve as a server; Start with a temporary directory, a free port and some
/// optional arguments then wait for a while for the server setup to complete.
#[fixture]
pub fn server<I>(#[default(&[] as &[&str])] args: I) -> TestServer
where
    I: IntoIterator + Clone,
    I::Item: AsRef<std::ffi::OsStr>,
{
    let port = port();
    let tmpdir = tmpdir();
    let mut child = Command::cargo_bin("miniserve")
        .expect("Couldn't find test binary")
        .arg(tmpdir.path())
        .arg("-v")
        .arg("-p")
        .arg(port.to_string())
        .args(args.clone())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Couldn't run test binary");
    let is_tls = args
        .into_iter()
        .any(|x| x.as_ref().to_str().unwrap().contains("tls"));

    // Read from stdout/stderr in the background and print/eprint everything read.
    // This dance is required to allow test output capturing to work as expected.
    // See https://github.com/rust-lang/rust/issues/92370 and https://github.com/rust-lang/rust/issues/90785

    let stdout = child.stdout.take().expect("Child process stdout is None");
    thread::spawn(move || {
        BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .for_each(|line| println!("[miniserve stdout] {line}"));
    });

    let stderr = child.stderr.take().expect("Child process stderr is None");
    thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .for_each(|line| eprintln!("[miniserve stderr] {line}"));
    });

    wait_for_port(port);
    TestServer::new(port, tmpdir, child, is_tls)
}

/// Wait a max of 1s for the port to become available.
fn wait_for_port(port: u16) {
    let start_wait = Instant::now();

    while !port_check::is_port_reachable(format!("localhost:{port}")) {
        sleep(Duration::from_millis(100));

        if start_wait.elapsed().as_secs() > 1 {
            panic!("timeout waiting for port {port}");
        }
    }
}

pub struct TestServer {
    port: u16,
    tmpdir: TempDir,
    child: Child,
    is_tls: bool,
}

#[allow(dead_code)]
impl TestServer {
    pub fn new(port: u16, tmpdir: TempDir, child: Child, is_tls: bool) -> Self {
        Self {
            port,
            tmpdir,
            child,
            is_tls,
        }
    }

    pub fn url(&self) -> Url {
        let protocol = if self.is_tls { "https" } else { "http" };
        Url::parse(&format!("{}://localhost:{}", protocol, self.port)).unwrap()
    }

    pub fn path(&self) -> &std::path::Path {
        self.tmpdir.path()
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.child.kill().expect("Couldn't kill test server");
        self.child.wait().unwrap();
    }
}

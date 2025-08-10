use reqwest::blocking::{Client, multipart};
use rstest::rstest;
use select::{
    document::Document,
    predicate::{Attr, Text},
};

mod fixtures;

use crate::fixtures::{DIRECTORY_SYMLINK, Error, TestServer, server};

/// This should work because the flags for uploading files and creating directories
/// are set, and the directory name and path are valid.
#[rstest]
fn creating_directories_works(
    #[with(&["--upload-files", "--mkdir"])] server: TestServer,
) -> Result<(), Error> {
    let test_directory_name = "hello";

    // Before creating, check whether the directory does not yet exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != test_directory_name));

    // Perform the actual creation.
    let create_action = parsed
        .find(Attr("id", "mkdir"))
        .next()
        .expect("Couldn't find element with id=mkdir")
        .attr("action")
        .expect("Directory form doesn't have action attribute");
    let form = multipart::Form::new();
    let part = multipart::Part::text(test_directory_name);
    let form = form.part("mkdir", part);

    let client = Client::new();
    client
        .post(server.url().join(create_action)?)
        .multipart(form)
        .send()?
        .error_for_status()?;

    // After creating, check whether the directory is now getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(
        parsed
            .find(Text)
            .any(|x| x.text() == test_directory_name.to_owned() + "/")
    );

    Ok(())
}

/// This should fail because the server does not allow for creating directories
/// as the flags for uploading files and creating directories are not set.
/// The directory name and path are valid.
#[rstest]
fn creating_directories_is_prevented(server: TestServer) -> Result<(), Error> {
    let test_directory_name = "hello";

    // Before creating, check whether the directory does not yet exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != test_directory_name));

    // Ensure the directory creation form is not present
    assert!(parsed.find(Attr("id", "mkdir")).next().is_none());

    // Then try to create anyway
    let form = multipart::Form::new();
    let part = multipart::Part::text(test_directory_name);
    let form = form.part("mkdir", part);

    let client = Client::new();
    // This should fail
    assert!(
        client
            .post(server.url().join("/upload?path=/")?)
            .multipart(form)
            .send()?
            .error_for_status()
            .is_err()
    );

    // After creating, check whether the directory is now getting listed (shouldn't).
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(
        parsed
            .find(Text)
            .all(|x| x.text() != test_directory_name.to_owned() + "/")
    );

    Ok(())
}

/// This should fail because directory creation through symlinks should not be possible
/// when the the no symlinks flag is set.
#[rstest]
fn creating_directories_through_symlinks_is_prevented(
    #[with(&["--upload-files", "--mkdir", "--no-symlinks"])] server: TestServer,
) -> Result<(), Error> {
    // Before attempting to create, ensure the symlink does not exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != DIRECTORY_SYMLINK));

    // Attempt to perform directory creation.
    let form = multipart::Form::new();
    let part = multipart::Part::text(DIRECTORY_SYMLINK);
    let form = form.part("mkdir", part);

    // This should fail
    assert!(
        Client::new()
            .post(
                server
                    .url()
                    .join(format!("/upload?path=/{DIRECTORY_SYMLINK}").as_str())?
            )
            .multipart(form)
            .send()?
            .error_for_status()
            .is_err()
    );

    Ok(())
}

/// Test for path traversal vulnerability (CWE-22) in both path parameter of query string and in
/// mkdir name (Content-Disposition)
///
/// see: https://github.com/svenstaro/miniserve/issues/518
#[rstest]
#[case("foo", "bar", "foo/bar")] // Not CWE-22, but `foo` isn't a directory
#[case("/../foo", "bar", "foo/bar")]
#[case("/foo", "/../bar", "foo/bar")]
#[case("C:/foo", "C:/bar", if cfg!(windows) { "foo/bar" } else { "C:/foo/C:/bar" })]
#[case(r"C:\foo", r"C:\bar", if cfg!(windows) { "foo/bar" } else { r"C:\foo/C:\bar" })]
#[case(r"\foo", r"\..\bar", if cfg!(windows) { "foo/bar" } else { r"\foo/\..\bar" })]
fn prevent_path_transversal_attacks(
    #[with(&["--upload-files", "--mkdir"])] server: TestServer,
    #[case] path: &str,
    #[case] dir_name: &'static str,
    #[case] expected: &str,
) -> Result<(), Error> {
    let expected_path = server.path().join(expected);
    assert!(!expected_path.exists());

    let form = multipart::Form::new();
    let part = multipart::Part::text(dir_name);
    let form = form.part("mkdir", part);

    // This should fail
    assert!(
        Client::new()
            .post(server.url().join(&format!("/upload/path={path}"))?)
            .multipart(form)
            .send()?
            .error_for_status()
            .is_err()
    );

    Ok(())
}

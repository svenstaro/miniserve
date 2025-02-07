use std::fs::create_dir_all;
use std::path::Path;

use assert_fs::fixture::TempDir;
use reqwest::blocking::{multipart, Client};
use rstest::rstest;
use select::document::Document;
use select::predicate::{Attr, Text};

mod fixtures;

use crate::fixtures::{server, tmpdir, Error, TestServer};

#[rstest]
fn uploading_files_works(#[with(&["-u"])] server: TestServer) -> Result<(), Error> {
    let test_file_name = "uploaded test file.txt";

    // Before uploading, check whether the uploaded file does not yet exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != test_file_name));

    // Perform the actual upload.
    let upload_action = parsed
        .find(Attr("id", "file_submit"))
        .next()
        .expect("Couldn't find element with id=file_submit")
        .attr("action")
        .expect("Upload form doesn't have action attribute");
    let form = multipart::Form::new();
    let part = multipart::Part::text("this should be uploaded")
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    client
        .post(server.url().join(upload_action)?)
        .multipart(form)
        .send()?
        .error_for_status()?;

    // After uploading, check whether the uploaded file is now getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));

    Ok(())
}

#[rstest]
fn uploading_files_is_prevented(server: TestServer) -> Result<(), Error> {
    let test_file_name = "uploaded test file.txt";

    // Before uploading, check whether the uploaded file does not yet exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != test_file_name));

    // Ensure the file upload form is not present
    assert!(parsed.find(Attr("id", "file_submit")).next().is_none());

    // Then try to upload anyway
    let form = multipart::Form::new();
    let part = multipart::Part::text("this should not be uploaded")
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    // Ensure uploading fails and returns an error
    assert!(client
        .post(server.url().join("/upload?path=/")?)
        .multipart(form)
        .send()?
        .error_for_status()
        .is_err());

    // After uploading, check whether the uploaded file is NOT getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(!parsed.find(Text).any(|x| x.text() == test_file_name));

    Ok(())
}

/// This test runs the server with --allowed-upload-dir argument and
/// checks that file upload to a different directory is actually prevented.
#[rstest]
#[case(server(&["-u", "someDir"]))]
#[case(server(&["-u", "someDir/some_sub_dir"]))]
fn uploading_files_is_restricted(#[case] server: TestServer) -> Result<(), Error> {
    let test_file_name = "uploaded test file.txt";

    // Then try to upload file to root directory (which is not the --allowed-upload-dir)
    let form = multipart::Form::new();
    let part = multipart::Part::text("this should not be uploaded")
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    // Ensure uploading fails and returns an error
    assert_eq!(
        403,
        client
            .post(server.url().join("/upload?path=/")?)
            .multipart(form)
            .send()?
            .status()
    );

    // After uploading, check whether the uploaded file is NOT getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(!parsed.find(Text).any(|x| x.text() == test_file_name));

    Ok(())
}

/// This tests that we can upload files to the directory specified by --allow-upload-dir
#[rstest]
#[case(server(&["-u", "someDir"]), vec!["someDir"])]
#[case(server(&["-u", "./-someDir"]), vec!["./-someDir"])]
#[case(server(&["-u", Path::new("someDir/some_sub_dir").to_str().unwrap()]),
  vec!["someDir/some_sub_dir"])]
#[case(server(&["-u", Path::new("someDir/some_sub_dir").to_str().unwrap(),
                "-u", Path::new("someDir/some_other_dir").to_str().unwrap()]),
       vec!["someDir/some_sub_dir", "someDir/some_other_dir"])]
fn uploading_files_to_allowed_dir_works(
    #[case] server: TestServer,
    #[case] upload_dirs: Vec<&str>,
) -> Result<(), Error> {
    let test_file_name = "uploaded test file.txt";

    for upload_dir in upload_dirs {
        // Create test directory
        create_dir_all(server.path().join(Path::new(upload_dir))).unwrap();

        // Before uploading, check whether the uploaded file does not yet exist.
        let body = reqwest::blocking::get(server.url().join(upload_dir)?)?.error_for_status()?;
        let parsed = Document::from_read(body)?;
        assert!(parsed.find(Text).all(|x| x.text() != test_file_name));

        // Perform the actual upload.
        let upload_action = parsed
            .find(Attr("id", "file_submit"))
            .next()
            .expect("Couldn't find element with id=file_submit")
            .attr("action")
            .expect("Upload form doesn't have action attribute");
        let form = multipart::Form::new();
        let part = multipart::Part::text("this should be uploaded")
            .file_name(test_file_name)
            .mime_str("text/plain")?;
        let form = form.part("file_to_upload", part);

        let client = Client::new();
        client
            .post(server.url().join(upload_action)?)
            .multipart(form)
            .send()?
            .error_for_status()?;

        // After uploading, check whether the uploaded file is now getting listed.
        let body = reqwest::blocking::get(server.url().join(upload_dir)?)?;
        let parsed = Document::from_read(body)?;
        assert!(parsed.find(Text).any(|x| x.text() == test_file_name));
    }
    Ok(())
}

/// Test for path traversal vulnerability (CWE-22) in both path parameter of query string and in
/// file name (Content-Disposition)
///
/// see: https://github.com/svenstaro/miniserve/issues/518
#[rstest]
#[case("foo", "bar", "foo/bar")]
#[case("/../foo", "bar", "foo/bar")]
#[case("/foo", "/../bar", "foo/bar")]
#[case("C:/foo", "C:/bar", if cfg!(windows) { "foo/bar" } else { "C:/foo/C:/bar" })]
#[case(r"C:\foo", r"C:\bar", if cfg!(windows) { "foo/bar" } else { r"C:\foo/C:\bar" })]
#[case(r"\foo", r"\..\bar", if cfg!(windows) { "foo/bar" } else { r"\foo/\..\bar" })]
fn prevent_path_traversal_attacks(
    #[with(&["-u"])] server: TestServer,
    #[case] path: &str,
    #[case] filename: &'static str,
    #[case] expected: &str,
) -> Result<(), Error> {
    // Create test directories
    create_dir_all(server.path().join("foo")).unwrap();
    if !cfg!(windows) {
        for dir in &["C:/foo/C:", r"C:\foo", r"\foo"] {
            create_dir_all(server.path().join(dir))
                .unwrap_or_else(|_| panic!("failed to create: {dir:?}"));
        }
    }

    let expected_path = server.path().join(expected);
    assert!(!expected_path.exists());

    // Perform the actual upload.
    let part = multipart::Part::text("this should be uploaded")
        .file_name(filename)
        .mime_str("text/plain")?;
    let form = multipart::Form::new().part("file_to_upload", part);

    Client::new()
        .post(server.url().join(&format!("/upload?path={path}"))?)
        .multipart(form)
        .send()?
        .error_for_status()?;

    // Make sure that the file was uploaded to the expected path
    assert!(expected_path.exists());

    Ok(())
}

/// Test uploading to symlink directories that point outside the server root.
/// See https://github.com/svenstaro/miniserve/issues/466
#[rstest]
#[case(server(&["-u"]), true)]
#[case(server(&["-u", "--no-symlinks"]), false)]
fn upload_to_symlink_directory(
    #[case] server: TestServer,
    #[case] ok: bool,
    tmpdir: TempDir,
) -> Result<(), Error> {
    #[cfg(unix)]
    use std::os::unix::fs::symlink as symlink_dir;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir;

    // Create symlink directory "foo" to point outside the root
    let (dir, filename) = ("foo", "bar");
    symlink_dir(tmpdir.path(), server.path().join(dir)).unwrap();

    let full_path = server.path().join(dir).join(filename);
    assert!(!full_path.exists());

    // Try to upload
    let part = multipart::Part::text("this should be uploaded")
        .file_name(filename)
        .mime_str("text/plain")?;
    let form = multipart::Form::new().part("file_to_upload", part);

    let status = Client::new()
        .post(server.url().join(&format!("/upload?path={dir}"))?)
        .multipart(form)
        .send()?
        .error_for_status();

    // Make sure upload behave as expected
    assert_eq!(status.is_ok(), ok);
    assert_eq!(full_path.exists(), ok);

    Ok(())
}

/// Test setting the HTML accept attribute using -m and -M.
#[rstest]
#[case(server(&["-u"]), None)]
#[case(server(&["-u", "-m", "image"]), Some("image/*"))]
#[case(server(&["-u", "-m", "image", "-m", "audio", "-m", "video"]), Some("image/*,audio/*,video/*"))]
#[case(server(&["-u", "-m", "audio", "-m", "image", "-m", "video"]), Some("audio/*,image/*,video/*"))]
#[case(server(&["-u", "-M", "test_value"]), Some("test_value"))]
fn set_media_type(
    #[case] server: TestServer,
    #[case] expected_accept_value: Option<&str>,
) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    let input = parsed.find(Attr("id", "file-input")).next().unwrap();
    assert_eq!(input.attr("accept"), expected_accept_value);

    Ok(())
}

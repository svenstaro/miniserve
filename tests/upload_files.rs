use std::fs::create_dir_all;
use std::path::Path;

use assert_fs::fixture::TempDir;
use reqwest::blocking::{Client, multipart};
use reqwest::header::HeaderMap;
use rstest::rstest;
use select::document::Document;
use select::predicate::{Attr, Text};

mod fixtures;

use crate::fixtures::{Error, TestServer, server, tmpdir};

// Generate the hashes using the following
// ```bash
// $ sha256 -s 'this should be uploaded'
// $ sha512 -s 'this should be uploaded'
// ```
#[rstest]
#[case::no_hash(None, None)]
#[case::only_hash(None, Some("test"))]
#[case::partial_sha256_hash(Some("SHA256"), None)]
#[case::partial_sha512_hash(Some("SHA512"), None)]
#[case::sha256_hash(
    Some("SHA256"),
    Some("e37b14e22e7b3f50dadaf821c189af80f79b1f39fd5a8b3b4f536103735d4620")
)]
#[case::sha512_hash(
    Some("SHA512"),
    Some(
        "03bcfc52c53904e34e06b95e8c3ee1275c66960c441417892e977d52687e28afae85b6039509060ee07da739e4e7fc3137acd142162c1456f723604f8365e154"
    )
)]
fn uploading_files_works(
    #[with(&["-u"])] server: TestServer,
    #[case] sha_func: Option<&str>,
    #[case] sha: Option<&str>,
) -> Result<(), Error> {
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

    let mut headers = HeaderMap::new();
    if let Some(sha_func) = sha_func.as_ref() {
        headers.insert("X-File-Hash-Function", sha_func.parse()?);
    }
    if let Some(sha) = sha.as_ref() {
        headers.insert("X-File-Hash", sha.parse()?);
    }

    let client = Client::builder().default_headers(headers).build()?;

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
    assert!(
        client
            .post(server.url().join("/upload?path=/")?)
            .multipart(form)
            .send()?
            .error_for_status()
            .is_err()
    );

    // After uploading, check whether the uploaded file is NOT getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(!parsed.find(Text).any(|x| x.text() == test_file_name));

    Ok(())
}

// Generated hashs with the following
// ```bash
// echo "invalid" | base64 | sha256
// echo "invalid" | base64 | sha512
// ```
#[rstest]
#[case::sha256_hash(
    Some("SHA256"),
    Some("f4ddf641a44e8fe8248cc086532cafaa8a914a21a937e40be67926ea074b955a")
)]
#[case::sha512_hash(
    Some("SHA512"),
    Some(
        "d3fe39ab560dd7ba91e6e2f8c948066d696f2afcfc90bf9df32946512f6934079807f301235b88b72bf746b6a88bf111bc5abe5c711514ed0731d286985297ba"
    )
)]
#[case::sha128_hash(Some("SHA128"), Some("invalid"))]
fn uploading_files_with_invalid_sha_func_is_prevented(
    #[with(&["-u"])] server: TestServer,
    #[case] sha_func: Option<&str>,
    #[case] sha: Option<&str>,
) -> Result<(), Error> {
    let test_file_name = "uploaded test file.txt";

    // Before uploading, check whether the uploaded file does not yet exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != test_file_name));

    // Perform the actual upload.
    let form = multipart::Form::new();
    let part = multipart::Part::text("this should be uploaded")
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let mut headers = HeaderMap::new();
    if let Some(sha_func) = sha_func.as_ref() {
        headers.insert("X-File-Hash-Function", sha_func.parse()?);
    }
    if let Some(sha) = sha.as_ref() {
        headers.insert("X-File-Hash", sha.parse()?);
    }

    let client = Client::builder().default_headers(headers).build()?;

    assert!(
        client
            .post(server.url().join("/upload?path=/")?)
            .multipart(form)
            .send()?
            .error_for_status()
            .is_err()
    );

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

#[rstest]
#[case(server(&["-u"]))]
#[case(server(&["-u", "-o", "error"]))]
#[case(server(&["-u", "--on-duplicate-files", "error"]))]
fn uploading_duplicate_file_is_prevented(#[case] server: TestServer) -> Result<(), Error> {
    let test_file_name = "duplicate test file.txt";
    let test_file_contents = "Test File Contents";
    let test_file_contents_new = "New Uploaded Test File Contents";

    // create the file
    let test_file_path = server.path().join(test_file_name);
    std::fs::write(&test_file_path, test_file_contents)?;

    // Before uploading, make sure the file is there.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));

    // Perform the actual upload.
    let upload_action = parsed
        .find(Attr("id", "file_submit"))
        .next()
        .expect("Couldn't find element with id=file_submit")
        .attr("action")
        .expect("Upload form doesn't have action attribute");
    // Then try to upload anyway
    let form = multipart::Form::new();
    let part = multipart::Part::text(test_file_contents_new)
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    // Ensure uploading fails and returns an error
    assert!(
        client
            .post(server.url().join(upload_action)?)
            .multipart(form)
            .send()?
            .error_for_status()
            .is_err()
    );

    // After uploading, uploaded file is still getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));
    // and assert the contents is the same as before
    assert_file_contents(&test_file_path, test_file_contents);

    Ok(())
}

#[rstest]
#[case(server(&["-u", "-o", "overwrite"]))]
#[case(server(&["-u", "--on-duplicate-files", "overwrite"]))]
fn overwrite_duplicate_file(#[case] server: TestServer) -> Result<(), Error> {
    let test_file_name = "duplicate test file.txt";
    let test_file_contents = "Test File Contents";
    let test_file_contents_new = "New Uploaded Test File Contents";

    // create the file
    let test_file_path = server.path().join(test_file_name);
    let _ = std::fs::write(&test_file_path, test_file_contents);

    // Before uploading, make sure the file is there.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));

    // Perform the actual upload.
    let upload_action = parsed
        .find(Attr("id", "file_submit"))
        .next()
        .expect("Couldn't find element with id=file_submit")
        .attr("action")
        .expect("Upload form doesn't have action attribute");
    // Then try to upload anyway
    let form = multipart::Form::new();
    let part = multipart::Part::text(test_file_contents_new)
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    client
        .post(server.url().join(upload_action)?)
        .multipart(form)
        .send()?
        .error_for_status()?;

    // After uploading, verify the listing has the file
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));
    // and assert the contents is from recently uploaded file
    assert_file_contents(&test_file_path, test_file_contents_new);

    Ok(())
}

#[rstest]
#[case(server(&["-u", "-o", "rename"]))]
#[case(server(&["-u", "--on-duplicate-files", "rename"]))]
fn rename_duplicate_file(#[case] server: TestServer) -> Result<(), Error> {
    let test_file_name = "duplicate test file.txt";
    let test_file_contents = "Test File Contents";
    let test_file_name_new = "duplicate test file-1.txt";
    let test_file_contents_new = "New Uploaded Test File Contents";

    // create the file
    let test_file_path = server.path().join(test_file_name);
    let _ = std::fs::write(&test_file_path, test_file_contents);

    // Before uploading, make sure the file is there.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));

    // Perform the actual upload.
    let upload_action = parsed
        .find(Attr("id", "file_submit"))
        .next()
        .expect("Couldn't find element with id=file_submit")
        .attr("action")
        .expect("Upload form doesn't have action attribute");
    // Then try to upload anyway
    let form = multipart::Form::new();
    let part = multipart::Part::text(test_file_contents_new)
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    client
        .post(server.url().join(upload_action)?)
        .multipart(form)
        .send()?
        .error_for_status()?;

    // After uploading, assert the old file is still getting listed, and the new file is also in listing
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name_new));
    // and assert the contents is the same as before for old file, and new contents for new file
    assert_file_contents(&test_file_path, test_file_contents);
    assert_file_contents(
        &server.path().join(test_file_name_new),
        test_file_contents_new,
    );

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

fn assert_file_contents(file_path: &Path, contents: &str) {
    let file_contents = std::fs::read_to_string(file_path).unwrap();
    assert!(file_contents == contents)
}

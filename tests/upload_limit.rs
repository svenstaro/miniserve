use std::fs;
use std::io::{self, Cursor, Read};

use assert_cmd::{Command, cargo};
use assert_fs::TempDir;
use reqwest::StatusCode;
use reqwest::blocking::{Client, multipart};
use rstest::rstest;
use select::{document::Document, predicate::Attr};

mod fixtures;
use fixtures::{Error, TestServer, reqwest_client, server};

struct SmallChunks(Cursor<Vec<u8>>);

impl Read for SmallChunks {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let length = buffer.len().min(3);
        self.0.read(&mut buffer[..length])
    }
}

#[rstest]
#[case::empty(0, true)]
#[case::below_limit(9, true)]
#[case::exact_limit(10, true)]
#[case::above_limit(11, false)]
#[case::large(65536, false)]
fn upload_limits_apply_per_file(
    #[with(&["-u", "--max-upload-size=10B"])] server: TestServer,
    reqwest_client: Client,
    #[case] size: usize,
    #[case] accepted: bool,
    #[values(false, true)] chunked: bool,
) -> Result<(), Error> {
    let data = vec![b'x'; size];
    let part = if chunked {
        multipart::Part::reader(SmallChunks(Cursor::new(data.clone())))
    } else {
        multipart::Part::bytes(data.clone())
    };
    let response = reqwest_client
        .post(server.url().join("/upload?path=/")?)
        .multipart(multipart::Form::new().part("file", part.file_name("limit.txt")))
        .send()?;
    let path = server.path().join("limit.txt");
    if accepted {
        assert!(response.status().is_success());
        assert_eq!(fs::read(path)?, data);
    } else {
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(!path.exists());
    }
    Ok(())
}

#[rstest]
fn the_limit_is_not_the_total_multipart_request_size(
    #[with(&["-u", "--max-upload-size=10B"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let form = multipart::Form::new()
        .part(
            "first",
            multipart::Part::text("1234567890").file_name("first.txt"),
        )
        .part(
            "second",
            multipart::Part::text("abcdefghij").file_name("second.txt"),
        );
    reqwest_client
        .post(server.url().join("/upload?path=/")?)
        .multipart(form)
        .send()?
        .error_for_status()?;
    assert_eq!(fs::read(server.path().join("first.txt"))?, b"1234567890");
    assert_eq!(fs::read(server.path().join("second.txt"))?, b"abcdefghij");
    Ok(())
}

#[rstest]
fn oversized_upload_keeps_existing_file_and_cleans_temporary_data(
    reqwest_client: Client,
) -> Result<(), Error> {
    let temporary = TempDir::new()?;
    let server = server([
        "-u",
        "--max-upload-size=10B",
        "--on-duplicate-files=overwrite",
        "--temp-directory",
        temporary.path().to_str().unwrap(),
    ]);
    let path = server.path().join("test.txt");
    let original = fs::read(&path)?;
    let response = reqwest_client
        .post(server.url().join("/upload?path=/")?)
        .multipart(multipart::Form::new().part(
            "file",
            multipart::Part::bytes(vec![1; 1000]).file_name("test.txt"),
        ))
        .send()?;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(fs::read(path)?, original);
    assert_eq!(fs::read_dir(temporary.path())?.count(), 0);
    Ok(())
}

#[rstest]
fn zero_limit_allows_only_empty_files(
    #[with(&["-u", "--max-upload-size=0"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    for (name, data, accepted) in [("empty", "", true), ("nonempty", "x", false)] {
        let response = reqwest_client
            .post(server.url().join("/upload?path=/")?)
            .multipart(
                multipart::Form::new().part("file", multipart::Part::text(data).file_name(name)),
            )
            .send()?;
        assert_eq!(response.status().is_success(), accepted);
        assert_eq!(server.path().join(name).exists(), accepted);
    }
    Ok(())
}

#[rstest]
fn absent_limit_keeps_large_uploads_working(
    #[with(&["-u"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let data = vec![42; 2 * 1024 * 1024];
    reqwest_client
        .post(server.url().join("/upload?path=/")?)
        .multipart(multipart::Form::new().part(
            "file",
            multipart::Part::bytes(data.clone()).file_name("large.bin"),
        ))
        .send()?
        .error_for_status()?;
    assert_eq!(fs::read(server.path().join("large.bin"))?, data);
    Ok(())
}

#[rstest]
fn limit_is_exposed_to_the_upload_form(
    #[with(&["-u", "--max-upload-size=1KiB"])] server: TestServer,
    reqwest_client: Client,
) -> Result<(), Error> {
    let document = Document::from_read(reqwest_client.get(server.url()).send()?)?;
    let form = document.find(Attr("id", "file_submit")).next().unwrap();
    assert_eq!(form.attr("data-max-upload-size"), Some("1024"));
    Ok(())
}

#[rstest]
fn malformed_upload_never_persists_partial_data(reqwest_client: Client) -> Result<(), Error> {
    let temporary = TempDir::new()?;
    let server = server(["-u", "--temp-directory", temporary.path().to_str().unwrap()]);
    let response = reqwest_client
        .post(server.url().join("/upload?path=/")?)
        .header("Content-Type", "multipart/form-data; boundary=broken")
        .body("--broken\r\nContent-Disposition: form-data; name=\"file\"; filename=\"partial.txt\"\r\n\r\npartial data")
        .send()?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!server.path().join("partial.txt").exists());
    assert_eq!(fs::read_dir(temporary.path())?.count(), 0);
    Ok(())
}

#[test]
fn invalid_upload_limits_are_rejected() {
    for value in [
        "-1",
        "nonsense",
        "18446744073709551616B",
        "18446744073709551615KiB",
    ] {
        Command::new(cargo::cargo_bin!("miniserve"))
            .timeout(std::time::Duration::from_secs(5))
            .args(["-u", "--max-upload-size", value, "."])
            .assert()
            .code(2);
    }
    Command::new(cargo::cargo_bin!("miniserve"))
        .timeout(std::time::Duration::from_secs(5))
        .args(["--max-upload-size=1MiB", "."])
        .assert()
        .code(2);
}

#[test]
fn valid_upload_limit_sizes_are_accepted() {
    for value in ["0", "1.5MB", "1KiB", "18446744073709551615B"] {
        Command::new(cargo::cargo_bin!("miniserve"))
            .timeout(std::time::Duration::from_secs(5))
            .args([
                "-u",
                "--max-upload-size",
                value,
                "--print-completions",
                "bash",
            ])
            .assert()
            .success();
    }
}

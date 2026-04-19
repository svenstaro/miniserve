use std::fs;

use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use rstest::rstest;

mod fixtures;

use crate::fixtures::{Error, TestServer, reqwest_client, server};

fn write_utf8_file(server: &TestServer, filename: &str) {
    fs::write(server.path().join(filename), "你好，miniserve\n")
        .expect("Couldn't write UTF-8 test file");
}

#[rstest]
#[case("test.txt", "text/plain; charset=utf-8")]
#[case("test.js", "text/javascript; charset=utf-8")]
#[case("README.md", "text/markdown; charset=utf-8")]
fn served_text_files_include_utf8_charset(
    server: TestServer,
    reqwest_client: Client,
    #[case] filename: &str,
    #[case] expected_content_type: &str,
) -> Result<(), Error> {
    write_utf8_file(&server, filename);

    let response = reqwest_client
        .get(server.url().join(filename)?)
        .send()?
        .error_for_status()?;

    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .expect("Expected Content-Type header")
            .to_str()?,
        expected_content_type,
    );

    let body = response.text()?;
    assert!(body.contains("你好，miniserve"));

    Ok(())
}

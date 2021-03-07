mod fixtures;
mod utils;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error, DEEPLY_NESTED_FILE, DIRECTORIES};
use pretty_assertions::{assert_eq, assert_ne};
use rstest::rstest;
use select::document::Document;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use url::Url;
use utils::get_link_from_text;

#[rstest(
    input,
    expected,
    case("", "/"),
    case("/dira", "/dira/"),
    case("/dirb/", "/dirb/"),
    case("/very/deeply/nested", "/very/deeply/nested/")
)]
/// Directories get a trailing slash.
fn index_gets_trailing_slash(
    tmpdir: TempDir,
    port: u16,
    input: &str,
    expected: &str,
) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let base_url = Url::parse(&format!("http://localhost:{}", port))?;
    let resp = reqwest::blocking::get(base_url.join(input)?)?;
    assert!(resp.url().as_str().ends_with(expected));

    child.kill()?;

    Ok(())
}

#[rstest]
/// Can't navigate up the root.
fn cant_navigate_up_the_root(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // We're using curl for this as it has the option `--path-as-is` which doesn't normalize
    // invalid urls. A useful feature in this particular case.
    let base_url = Url::parse(&format!("http://localhost:{}", port))?;
    let curl_successful = Command::new("curl")
        .arg("-s")
        .arg("--fail")
        .arg("--path-as-is")
        .arg(format!("{}/../", base_url))
        .stdout(Stdio::null())
        .status()?
        .success();
    assert!(curl_successful);

    child.kill()?;

    Ok(())
}

#[rstest]
/// We can navigate into directories and back using shown links.
fn can_navigate_into_dirs_and_back(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let base_url = Url::parse(&format!("http://localhost:{}/", port))?;
    let initial_body = reqwest::blocking::get(base_url.as_str())?.error_for_status()?;
    let initial_parsed = Document::from_read(initial_body)?;
    for &directory in DIRECTORIES {
        let dir_elem = get_link_from_text(&initial_parsed, &directory).expect("Dir not found.");
        let body =
            reqwest::blocking::get(&format!("{}{}", base_url, dir_elem))?.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let back_link =
            get_link_from_text(&parsed, "Parent directory").expect("Back link not found.");
        let resp = reqwest::blocking::get(&format!("{}{}", base_url, back_link))?;

        // Now check that we can actually get back to the original location we came from using the
        // link.
        assert_eq!(resp.url().as_str(), base_url.as_str());
    }

    child.kill()?;

    Ok(())
}

#[rstest]
/// We can navigate deep into the file tree and back using shown links.
fn can_navigate_deep_into_dirs_and_back(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // Create a vector of directory names. We don't need to fetch the file and so we'll
    // remove that part.
    let dir_names = {
        let mut comps = DEEPLY_NESTED_FILE
            .split("/")
            .map(|d| format!("{}/", d))
            .collect::<Vec<String>>();
        comps.pop();
        comps
    };
    let base_url = Url::parse(&format!("http://localhost:{}/", port))?;

    // First we'll go forwards through the directory tree and then we'll go backwards.
    // In the end, we'll have to end up where we came from.
    let mut next_url = base_url.clone();
    for dir_name in dir_names.iter() {
        let resp = reqwest::blocking::get(next_url.as_str())?;
        let body = resp.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let dir_elem = get_link_from_text(&parsed, &dir_name).expect("Dir not found.");
        next_url = next_url.join(&dir_elem)?;
    }
    assert_ne!(base_url, next_url);

    // Now try to get out the tree again using links only.
    while next_url != base_url {
        let resp = reqwest::blocking::get(next_url.as_str())?;
        let body = resp.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let dir_elem =
            get_link_from_text(&parsed, "Parent directory").expect("Back link not found.");
        next_url = next_url.join(&dir_elem)?;
    }
    assert_eq!(base_url, next_url);

    child.kill()?;

    Ok(())
}

#[rstest(use_custom_title, case(true), case(false))]
/// We can use breadcrumbs to navigate.
fn can_navigate_using_breadcrumbs(
    tmpdir: TempDir,
    port: u16,
    use_custom_title: bool,
) -> Result<(), Error> {
    let mut command_base = Command::cargo_bin("miniserve")?;
    let mut command = command_base.arg("-p").arg(port.to_string());

    if use_custom_title {
        command = command.arg("--title").arg("some title")
    }

    let mut child = command.arg(tmpdir.path()).stdout(Stdio::null()).spawn()?;

    sleep(Duration::from_secs(1));

    // Create a vector of directory names. We don't need to fetch the file and so we'll
    // remove that part.
    let dir: String = {
        let mut comps = DEEPLY_NESTED_FILE
            .split("/")
            .map(|d| format!("{}/", d))
            .collect::<Vec<String>>();
        comps.pop();
        comps.join("")
    };

    let base_url = Url::parse(&format!("http://localhost:{}/", port))?;
    let nested_url = base_url.join(&dir)?;

    let resp = reqwest::blocking::get(nested_url.as_str())?;
    let body = resp.error_for_status()?;
    let parsed = Document::from_read(body)?;

    let title_name = if use_custom_title {
        "some title".to_string()
    } else {
        format!("localhost:{}", port)
    };

    // can go back to root dir by clicking title
    let title_link = get_link_from_text(&parsed, &title_name).expect("Root dir link not found.");
    assert_eq!("/", title_link);

    // can go to intermediate dir
    let intermediate_dir_link =
        get_link_from_text(&parsed, "very").expect("Intermediate dir link not found.");
    assert_eq!("/very/", intermediate_dir_link);

    // current dir is not linked
    let current_dir_link = get_link_from_text(&parsed, "nested");
    assert_eq!(None, current_dir_link);

    child.kill()?;

    Ok(())
}

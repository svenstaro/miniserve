mod fixtures;
mod utils;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error, DIRECTORIES, DEEPLY_NESTED_FILE};
use rstest::rstest;
use select::document::Document;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use utils::get_link_from_text;

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

    let original_location = format!("http://localhost:{}/", port);
    let body = reqwest::get(&original_location)?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &directory in DIRECTORIES {
        let dir_elem = get_link_from_text(&parsed, &directory).expect("Dir should have been here.");
        let dir_body =
            reqwest::get(format!("http://localhost:{}/{}", port, dir_elem).as_str())?.error_for_status()?;
        let dir_parsed = Document::from_read(dir_body)?;
        let back_link = get_link_from_text(&dir_parsed, "Parent directory").expect("Back link should be there.");
        let back_location = format!("http://localhost:{}{}", port, back_link);

        // Now check that we can actually get back to the original location we came from using the
        // link.
        assert_eq!(back_location, original_location);
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
        let mut comps = DEEPLY_NESTED_FILE.split("/").map(|d| format!("{}/", d)).collect::<Vec<String>>();
        comps.pop();
        comps
    };
    let base_url = format!("http://localhost:{}", port);

    // First we'll go forwards through the directory tree and then we'll go backwards.
    // In the end, we'll have to end up where we came from.
    let mut next_url = base_url.clone();
    for dir_name in dir_names.iter() {
        let body = reqwest::get(&next_url)?.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let dir_elem = get_link_from_text(&parsed, &dir_name).expect("Dir should have been here.");
        next_url = format!("{}{}", base_url, dir_elem);
    }

    // Now try to get out the tree again using links only.
    let start_url = format!("{}/", base_url);
    while next_url != start_url {
        let body = reqwest::get(&next_url)?.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let dir_elem = get_link_from_text(&parsed, "Parent directory").expect("Back link not found.");
        next_url = format!("{}{}", base_url, dir_elem);
    }

    child.kill()?;

    Ok(())
}

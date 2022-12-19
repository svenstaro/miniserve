mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{
    port, server, server_no_stderr, tmpdir, Error, TestServer, DIRECTORIES, FILES,
    HIDDEN_DIRECTORIES, HIDDEN_FILES,
};
use http::StatusCode;
use regex::Regex;
use rstest::rstest;
use select::{document::Document, node::Node, predicate::Attr};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::{symlink as symlink_dir, symlink as symlink_file};
#[cfg(windows)]
use std::os::windows::fs::{symlink_dir, symlink_file};

#[rstest]
fn serves_requests_with_no_options(tmpdir: TempDir) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::blocking::get("http://localhost:8080")?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
    }
    for &dir in DIRECTORIES {
        assert!(parsed.find(|x: &Node| x.text() == dir).next().is_some());
    }

    child.kill()?;

    Ok(())
}

#[rstest]
fn serves_requests_with_non_default_port(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &file in FILES {
        let f = parsed.find(|x: &Node| x.text() == file).next().unwrap();
        reqwest::blocking::get(server.url().join(f.attr("href").unwrap())?)?.error_for_status()?;
        assert_eq!(
            format!("/{file}"),
            percent_encoding::percent_decode_str(f.attr("href").unwrap()).decode_utf8_lossy(),
        );
    }

    for &directory in DIRECTORIES {
        assert!(parsed
            .find(|x: &Node| x.text() == directory)
            .next()
            .is_some());
        let dir_body = reqwest::blocking::get(server.url().join(directory)?)?.error_for_status()?;
        let dir_body_parsed = Document::from_read(dir_body)?;
        for &file in FILES {
            assert!(dir_body_parsed
                .find(|x: &Node| x.text() == file)
                .next()
                .is_some());
        }
    }

    Ok(())
}

#[rstest]
fn serves_requests_hidden_files(#[with(&["--hidden"])] server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &file in FILES.iter().chain(HIDDEN_FILES) {
        let f = parsed.find(|x: &Node| x.text() == file).next().unwrap();
        assert_eq!(
            format!("/{file}"),
            percent_encoding::percent_decode_str(f.attr("href").unwrap()).decode_utf8_lossy(),
        );
    }

    for &directory in DIRECTORIES.iter().chain(HIDDEN_DIRECTORIES) {
        assert!(parsed
            .find(|x: &Node| x.text() == directory)
            .next()
            .is_some());
        let dir_body = reqwest::blocking::get(server.url().join(directory)?)?.error_for_status()?;
        let dir_body_parsed = Document::from_read(dir_body)?;
        for &file in FILES.iter().chain(HIDDEN_FILES) {
            assert!(dir_body_parsed
                .find(|x: &Node| x.text() == file)
                .next()
                .is_some());
        }
    }

    Ok(())
}

#[rstest]
fn serves_requests_no_hidden_files_without_flag(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &hidden_item in HIDDEN_FILES.iter().chain(HIDDEN_DIRECTORIES) {
        assert!(parsed
            .find(|x: &Node| x.text() == hidden_item)
            .next()
            .is_none());
        let resp = reqwest::blocking::get(server.url().join(hidden_item)?)?;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    Ok(())
}

#[rstest]
#[case(true, false, server(&["--no-symlinks"]))]
#[case(true, true, server(&["--no-symlinks", "--show-symlink-info"]))]
#[case(false, false, server(None::<&str>))]
fn serves_requests_symlinks(
    #[case] no_symlinks: bool,
    #[case] show_symlink_info: bool,
    #[case] server: TestServer,
) -> Result<(), Error> {
    let file = "symlink-file.html";
    let dir = "symlink-dir/";
    let broken = "symlink broken";

    // Set up some basic symlinks:
    // to dir, to file, to non-existent location
    let orig = DIRECTORIES[0].strip_suffix('/').unwrap();
    let link = server.path().join(dir.strip_suffix('/').unwrap());
    symlink_dir(orig, link).expect("Couldn't create symlink");
    symlink_file(FILES[0], server.path().join(file)).expect("Couldn't create symlink");
    symlink_file("should-not-exist.xxx", server.path().join(broken))
        .expect("Couldn't create symlink");

    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &entry in &[file, dir] {
        let status = reqwest::blocking::get(server.url().join(entry)?)?.status();
        // We expect a 404 here for when `no_symlinks` is `true`.
        if no_symlinks {
            assert_eq!(status, StatusCode::NOT_FOUND);
        } else {
            assert_eq!(status, StatusCode::OK);
        }

        let node = parsed
            .find(|x: &Node| x.name().unwrap_or_default() == "a" && x.text() == entry)
            .next();

        // If symlinks are deactivated, none should be shown in the listing.
        assert_eq!(node.is_none(), no_symlinks);
        if node.is_some() && show_symlink_info {
            assert_eq!(node.unwrap().attr("class").unwrap(), "symlink");
        }

        // If following symlinks is deactivated, we can just skip this iteration as we assorted
        // above the no entries in the listing can be found for symlinks in that case.
        if no_symlinks {
            continue;
        }

        let node = node.unwrap();
        assert_eq!(node.attr("href").unwrap().strip_prefix('/').unwrap(), entry);
        if entry.ends_with('/') {
            let node = parsed
                .find(|x: &Node| x.name().unwrap_or_default() == "a" && x.text() == DIRECTORIES[0])
                .next();
            assert_eq!(node.unwrap().attr("class").unwrap(), "directory");
        } else {
            let node = parsed
                .find(|x: &Node| x.name().unwrap_or_default() == "a" && x.text() == FILES[0])
                .next();
            assert_eq!(node.unwrap().attr("class").unwrap(), "file");
        }
    }
    assert!(parsed.find(|x: &Node| x.text() == broken).next().is_none());

    Ok(())
}

#[rstest]
fn serves_requests_with_randomly_assigned_port(tmpdir: TempDir) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg("0")
        .stdout(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));
    child.kill()?;

    let output = child.wait_with_output().expect("Failed to read stdout");
    let all_text = String::from_utf8(output.stdout)?;

    let re = Regex::new(r"http://127.0.0.1:(\d+)").unwrap();
    let caps = re.captures(all_text.as_str()).unwrap();
    let port_num = caps.get(1).unwrap().as_str().parse::<u16>().unwrap();

    assert!(port_num > 0);

    Ok(())
}

#[rstest]
fn serves_requests_custom_index_notice(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("--index=not.html")
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));

    child.kill()?;
    let output = child.wait_with_output().expect("Failed to read stdout");
    let all_text = String::from_utf8(output.stdout);

    assert!(
        all_text?.contains("The file 'not.html' provided for option --index could not be found.")
    );

    Ok(())
}

#[rstest]
#[case(server_no_stderr(&["--index", FILES[0]]))]
#[case(server_no_stderr(&["--index", "does-not-exist.html"]))]
fn index_fallback_to_listing(#[case] server: TestServer) -> Result<(), Error> {
    // If index file is not found, show directory listing instead both cases should return `Ok`
    reqwest::blocking::get(server.url())?.error_for_status()?;

    Ok(())
}

#[rstest]
#[case(server_no_stderr(&["--spa", "--index", FILES[0]]), "/")]
#[case(server_no_stderr(&["--spa", "--index", FILES[0]]), "/spa-route")]
#[case(server_no_stderr(&["--index", FILES[0]]), "/")]
fn serve_index_instead_of_404_in_spa_mode(
    #[case] server: TestServer,
    #[case] url: &str,
) -> Result<(), Error> {
    let body = reqwest::blocking::get(format!("{}{}", server.url(), url))?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed
        .find(|x: &Node| x.text() == "Test Hello Yes")
        .next()
        .is_some());

    Ok(())
}

#[rstest]
#[case(server(&["--route-prefix", "foobar"]))]
#[case(server(&["--route-prefix", "/foobar/"]))]
fn serves_requests_with_route_prefix(#[case] server: TestServer) -> Result<(), Error> {
    let url_without_route = server.url();
    let status = reqwest::blocking::get(url_without_route)?.status();
    assert_eq!(status, StatusCode::NOT_FOUND);

    let url_with_route = server.url().join("foobar")?;
    let status = reqwest::blocking::get(url_with_route)?.status();
    assert_eq!(status, StatusCode::OK);

    Ok(())
}

#[rstest]
#[case(server_no_stderr(&[] as &[&str]), "/[a-f0-9]+")]
#[case(server_no_stderr(&["--random-route"]), "/[a-f0-9]+")]
#[case(server_no_stderr(&["--route-prefix", "foobar"]), "/foobar/[a-f0-9]+")]
fn serves_requests_static_file_check(
    #[case] server: TestServer,
    #[case] static_file_pattern: String,
) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    let re = Regex::new(&static_file_pattern).unwrap();

    assert!(parsed
        .find(Attr("rel", "stylesheet"))
        .all(|x| re.is_match(x.attr("href").unwrap())));
    assert!(parsed
        .find(Attr("rel", "icon"))
        .all(|x| re.is_match(x.attr("href").unwrap())));

    Ok(())
}

mod fixtures;
mod utils;

use crate::fixtures::TestServer;
use assert_cmd::prelude::*;
use fixtures::{server, Error};
use pretty_assertions::assert_eq;
use reqwest::blocking::Client;
use rstest::rstest;
use select::document::Document;
use select::predicate::Class;
use select::predicate::Name;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest]
/// The ui displays the correct wget command to download the folder recursively
fn ui_displays_wget_element(server: TestServer) -> Result<(), Error> {
    let client = Client::new();

    let body = client.get(server.url()).send()?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    let wget_url = parsed
        .find(Class("downloadDirectory"))
        .next()
        .unwrap()
        .find(Class("cmd"))
        .next()
        .unwrap()
        .text();
    assert_eq!(
        wget_url,
        format!(
            "wget -r -c -nH -np --cut-dirs=0 -R \"index.html*\" \"{}?raw=true\"",
            server.url()
        )
    );

    let body = client
        .get(format!("{}/very/deeply/nested/", server.url()))
        .send()?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    let wget_url = parsed
        .find(Class("downloadDirectory"))
        .next()
        .unwrap()
        .find(Class("cmd"))
        .next()
        .unwrap()
        .text();
    assert_eq!(
        wget_url,
        format!(
            "wget -r -c -nH -np --cut-dirs=2 -R \"index.html*\" \"{}very/deeply/nested/?raw=true\"",
            server.url()
        )
    );

    Ok(())
}

#[rstest]
/// All hrefs in raw mode are links to directories or files & directories end with ?raw=true
fn raw_mode_links_to_directories_end_with_raw_true(server: TestServer) -> Result<(), Error> {
    fn verify_a_tags(parsed: Document) {
        // Ensure all links end with ?raw=true or are files
        for node in parsed.find(Name("a")) {
            if let Some(class) = node.attr("class") {
                if class == "root" || class == "directory" {
                    assert!(node.attr("href").unwrap().ends_with("?raw=true"));
                } else if class == "file" {
                    assert!(true);
                } else {
                    println!(
                        "This node is a link and neither of class directory, root or file: {:?}",
                        node
                    );
                    assert!(false);
                }
            }
        }
    }

    let urls = [
        format!("{}?raw=true", server.url()),
        format!("{}very/?raw=true", server.url()),
        format!("{}very/deeply/?raw=true", server.url()),
        format!("{}very/deeply/nested/?raw=true", server.url()),
    ];

    let client = Client::new();
    // Ensure the links to the archives are not present
    for url in urls.iter() {
        let body = client.get(url).send()?.error_for_status()?;
        let parsed = Document::from_read(body)?;
        verify_a_tags(parsed);
    }

    Ok(())
}

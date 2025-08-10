use pretty_assertions::assert_eq;
use reqwest::blocking::Client;
use rstest::rstest;
use select::{
    document::Document,
    predicate::{Class, Name},
};

mod fixtures;

use crate::fixtures::{Error, TestServer, server};

/// The footer displays the correct wget command to download the folder recursively
// This test can't test all aspects of the wget footer,
// a more detailed unit test is available
#[rstest]
#[case(0, "")]
#[case(1, "dira/")]
#[case(2, "very/deeply/")]
#[case(3, "very/deeply/nested/")]
fn ui_displays_wget_element(
    #[case] depth: u8,
    #[case] dir: &str,
    #[with(&["--show-wget-footer"])] server: TestServer,
) -> Result<(), Error> {
    let client = Client::new();

    let body = client
        .get(format!("{}{}", server.url(), dir))
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
    let cut_dirs = match depth {
        // Put all the files in a folder of this name
        0 => format!(" -P 'localhost:{}'", server.port()),
        1 => String::new(),
        // Avoids putting the files in excessive directories
        x => format!(" --cut-dirs={}", x - 1),
    };
    assert_eq!(
        wget_url,
        format!(
            "wget -rcnp -R 'index.html*' -nH{} '{}{}?raw=true'",
            cut_dirs,
            server.url(),
            dir
        )
    );

    Ok(())
}

/// All hrefs in raw mode are links to directories or files & directories end with ?raw=true
#[rstest]
#[case("")]
#[case("very/")]
#[case("very/deeply/")]
#[case("very/deeply/nested/")]
fn raw_mode_links_to_directories_end_with_raw_true(
    #[case] dir: &str,
    #[with(&["--show-wget-footer"])] server: TestServer,
) -> Result<(), Error> {
    fn verify_a_tags(parsed: Document) {
        // Ensure all links end with ?raw=true or are files
        for node in parsed.find(Name("a")) {
            if let Some(class) = node.attr("class") {
                if class == "root" || class == "directory" {
                    assert!(
                        node.attr("href").unwrap().ends_with("?raw=true"),
                        "doesn't end with ?raw=true"
                    );
                } else if class == "file" {
                    return;
                } else {
                    panic!(
                        "This node is a link and neither of class directory, root or file: {node:?}"
                    );
                }
            }
        }
    }

    let client = Client::new();
    // Ensure the links to the archives are not present
    let body = client
        .get(format!("{}{}?raw=true", server.url(), dir))
        .send()?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    verify_a_tags(parsed);

    Ok(())
}

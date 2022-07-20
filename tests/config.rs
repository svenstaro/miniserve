mod fixtures;

use fixtures::{server_no_stderr, Error, TestServer};
use regex::Regex;
use rstest::rstest;
use select::{document::Document, predicate::Attr};

#[rstest]
#[case(server_no_stderr(&[] as &[&str]), "/[a-f0-9]+")]
#[case(server_no_stderr(&["--random-route"]), "/[a-f0-9]+")]
#[case(server_no_stderr(&["--route-prefix", "foo"]), "/foo/[a-f0-9]+")]
fn check_static_file_route_pattern(
    #[case] server: TestServer,
    #[case] route_pattern: String,
) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    let re = Regex::new(&route_pattern).unwrap();

    assert!(parsed
        .find(Attr("rel", "stylesheet"))
        .all(|x| re.is_match(x.attr("href").unwrap())));
    assert!(parsed
        .find(Attr("rel", "icon"))
        .all(|x| re.is_match(x.attr("href").unwrap())));

    Ok(())
}

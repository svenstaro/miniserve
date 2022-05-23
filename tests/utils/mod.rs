#![allow(dead_code)]

use select::document::Document;
use select::node::Node;
use select::predicate::Name;
use select::predicate::Predicate;

/// Return the href attribute content for the closest anchor found by `text`.
pub fn get_link_from_text(document: &Document, text: &str) -> Option<String> {
    let a_elem = document
        .find(Name("a").and(|x: &Node| x.children().any(|x| x.text() == text)))
        .next()?;
    Some(a_elem.attr("href")?.to_string())
}

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

/// Return the href attributes of all links that start with the specified prefix `text`.
pub fn get_link_hrefs_from_text_with_prefix(document: &Document, text: &str) -> Vec<String> {
    let mut vec: Vec<String> = Vec::new();

    let a_elem = document.find(Name("a"));

    for element in a_elem {
        let str = element.attr("href").unwrap_or("");
        if str.to_string().starts_with(text) {
            vec.push(str.to_string());
        }
    }

    return vec;
}

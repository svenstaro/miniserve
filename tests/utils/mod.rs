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

/// Return the href attributes of all links that start with the specified `prefix`.
pub fn get_link_hrefs_with_prefix(document: &Document, prefix: &str) -> Vec<String> {
    let mut vec: Vec<String> = Vec::new();

    let a_elements = document.find(Name("a"));

    for element in a_elements {
        let s = element.attr("href").unwrap_or("");
        if s.to_string().starts_with(prefix) {
            vec.push(s.to_string());
        }
    }

    vec
}

use chrono::{DateTime, Duration, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use std::time::SystemTime;

use crate::listing;

/// Renders the file listing
pub fn page(
    page_title: &str,
    entries: Vec<listing::Entry>,
    is_root: bool,
    page_parent: Option<String>,
    sort_method: Option<listing::SortingMethod>,
    sort_order: Option<listing::SortingOrder>,
) -> Markup {
    html! {
        (page_header(page_title))
        body {
            h1 { (page_title) }
            table {
                thead {
                    th { (build_link("name", "Name", &sort_method, &sort_order)) }
                    th { (build_link("size", "Size", &sort_method, &sort_order)) }
                    th { (build_link("date", "Last modification", &sort_method, &sort_order)) }
                }
                tbody {
                    @if !is_root {
                        @if let Some(parent) = page_parent {
                            tr {
                                td {
                                    a.root href=(parent) {
                                        ".."
                                    }
                                }
                            }
                        }
                    }
                    @for entry in entries {
                        (entry_row(entry))
                    }
                }
            }
        }
    }
}

/// Partial: table header link
fn build_link(
    name: &str,
    title: &str,
    sort_method: &Option<listing::SortingMethod>,
    sort_order: &Option<listing::SortingOrder>,
) -> Markup {
    let mut link = format!("?sort={}&order=asc", name);
    let mut help = format!("Sort by {} in ascending order", name);

    if let Some(method) = sort_method {
        if method.to_string() == name {
            if let Some(order) = sort_order {
                if order.to_string() == "asc" {
                    link = format!("?sort={}&order=desc", name);
                    help = format!("Sort by {} in descending order", name);
                }
            }
        }
    };

    html! {
        a href=(link) title=(help) { (title) }
    }
}

/// Partial: page header
fn page_header(page_title: &str) -> Markup {
    html! {
        (DOCTYPE)
        html {
            meta charset="utf-8";
            meta http-equiv="X-UA-Compatible" content="IE=edge";
            meta name="viewport" content="width=device-width, initial-scale=1";
            title { (page_title) }
            style { (css()) }
        }
    }
}

/// Partial: row for an entry
fn entry_row(entry: listing::Entry) -> Markup {
    html! {
        tr {
            td {
                @if entry.is_dir() {
                    a.directory href=(entry.link) {
                        (entry.name) "/"
                    }
                } @else {
                    a.file href=(entry.link) {
                        (entry.name)
                    }
                }
                @if !entry.is_dir() {
                    @if let Some(size) = entry.size {
                        span .mobile-info {
                            strong { "Size: " }
                            (size)
                        }
                    }
                }
                span .mobile-info {
                    @if let Some(modification_date) = convert_to_utc(entry.last_modification_date) {
                        strong { "Last modification: " }
                        (modification_date.0) " "
                        (modification_date.1) " "
                    }
                    @if let Some(modification_timer) = humanize_systemtime(entry.last_modification_date) {
                        span .history { "(" (modification_timer) ")" }
                    }

                }
            }
            td {
                @if let Some(size) = entry.size {
                    (size)
                }
            }
            td.date-cell {
                @if let Some(modification_date) = convert_to_utc(entry.last_modification_date) {
                    span {
                        (modification_date.0)
                    }
                    span {
                        (modification_date.1)
                    }
                }
                @if let Some(modification_timer) = humanize_systemtime(entry.last_modification_date) {
                    span {
                        "(" (modification_timer) ")"
                    }
                }
            }
        }
    }
}

/// Partial: CSS
fn css() -> Markup {
    (PreEscaped(r#"
    body {
        margin: 0;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,"Helvetica Neue", Helvetica, Arial, sans-serif;
        font-weight: 300;
        color: #444444;
        padding: 0.125rem;
    }
    table {
        width: 100%;
        background: white;
        border: 0;
        table-layout: auto;
    }
    table thead {
        background: #efefef;
    }
    table tr th,
    table tr td {
        padding: 0.5625rem 0.625rem;
        font-size: 0.875rem;
        color: #777c82;
        text-align: left;
        line-height: 1.125rem;
        width: 33.333%;
    }
    table thead tr th {
        padding: 0.5rem 0.625rem 0.625rem;
        font-weight: bold;
        color: #444444;
    }
    table tr:nth-child(even) {
        background: #f6f6f6;
    }
    a {
        text-decoration: none;
        color: #3498db;
    }
    a.root, a.root:visited {
        font-weight: bold;
        color: #777c82;
    }
    a.directory {
        font-weight: bold;
    }
    a:hover {
        text-decoration: underline;
    }
    a:visited {
        color: #8e44ad;
    }
    td.date-cell {
        display: flex;
        width: calc(100% - 1.25rem);
    }
    td.date-cell span:first-of-type, 
    td.date-cell span:nth-of-type(2) {
        flex-basis:4.5rem;
    }
    td.date-cell span:nth-of-type(3), .history {
        color: #c5c5c5;
    }
    .file, .directory {
        display: block;
    }
    .mobile-info {
        display: none;
    }
    @media (max-width: 600px) {
        h1 {
            font-size: 1.375em;
        }
        td:not(:nth-child(1)), th:not(:nth-child(1)){
            display: none;
        }
        .mobile-info {
            display: block;
        }
        .file, .directory{
            padding-bottom: 0.5rem;
        }
    }
    @media (max-width: 400px) {
        h1 {
            font-size: 1.375em;
        }
    }"#.to_string()))
}

/// Converts a SystemTime object to a strings tuple (date, time)
/// Date is formatted as %e %b, e.g. Jul 12
/// Time is formatted as %R, e.g. 22:34
fn convert_to_utc(src_time: Option<SystemTime>) -> Option<(String, String)> {
    src_time.map(DateTime::<Utc>::from).map(|date_time| {
        (
            date_time.format("%e %b").to_string(),
            date_time.format("%R").to_string(),
        )
    })
}

/// Converts a SystemTime to a string readable by a human,
/// i.e. calculates the duration between now() and the given SystemTime,
/// and gives a rough approximation of the elapsed time since
fn humanize_systemtime(src_time: Option<SystemTime>) -> Option<String> {
    src_time
        .and_then(|std_time| SystemTime::now().duration_since(std_time).ok())
        .and_then(|from_now| Duration::from_std(from_now).ok())
        .map(|duration| HumanTime::from(duration).to_text_en(Accuracy::Rough, Tense::Past))
}

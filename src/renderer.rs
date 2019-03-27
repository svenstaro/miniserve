use chrono::{DateTime, Duration, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use std::time::SystemTime;

use crate::archive;
use crate::listing;

/// Renders the file listing
pub fn page(
    page_title: &str,
    entries: Vec<listing::Entry>,
    is_root: bool,
    page_parent: Option<String>,
    sort_method: Option<listing::SortingMethod>,
    sort_order: Option<listing::SortingOrder>,
    file_upload: bool,
    base: &str,
) -> Markup {
    html! {
        (page_header(page_title))
        body {
            span #top { }
            h1 { (page_title) }
            @if file_upload {
            form action={"/upload?path=" (base)} method="POST" enctype="multipart/form-data" {
                p { "Select file to upload" }
                input type="file" name="file_to_upload" {}
                input type="submit" value="Upload file" {}
            }
            }
            div.download {
                (archive_button(archive::CompressionMethod::TarGz))
            }
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
                                td colspan="3" {
                                    span.chevron { (chevron_left()) }
                                    a.root href=(parent) {
                                        "Parent directory"
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
            a.back href="#top" {
                (arrow_up())
            }
        }
    }
}

/// Partial: archive button
fn archive_button(compress_method: archive::CompressionMethod) -> Markup {
    let link = format!("?download={}", compress_method.to_string());
    let text = format!("Download .{}", compress_method.extension());

    html! {
        a href=(link) {
            (text)
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
    let mut chevron = chevron_up();
    let mut class = "";

    if let Some(method) = sort_method {
        if method.to_string() == name {
            class = "active";
            if let Some(order) = sort_order {
                if order.to_string() == "asc" {
                    link = format!("?sort={}&order=desc", name);
                    help = format!("Sort by {} in descending order", name);
                    chevron = chevron_down();
                }
            }
        }
    };

    html! {
        span class=(class) {
            span.chevron { (chevron) }
            a href=(link) title=(help) { (title) }
        }
    }
}

/// Partial: row for an entry
fn entry_row(entry: listing::Entry) -> Markup {
    html! {
        tr {
            td {
                p {
                    @if entry.is_dir() {
                        a.directory href=(entry.link) {
                            (entry.name) "/"
                        }
                    } @else {
                        a.file href=(entry.link) {
                            (entry.name)
                        }
                    }
                }
                @if !entry.is_dir() {
                    @if let Some(size) = entry.size {
                        span .mobile-info {
                            strong { "Size: " }
                            (size)
                            (br())
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
                        (br())
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
                        (modification_date.0) " "
                    }
                    span {
                        (modification_date.1) " "
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
    html {
        font-smoothing: antialiased;
        text-rendering: optimizeLegibility;
    }
    body {
        margin: 0;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,"Helvetica Neue", Helvetica, Arial, sans-serif;
        font-weight: 300;
        color: #444444;
        padding: 0.125rem;
    }
    strong {
        font-weight: bold;
    }
    p {
        margin: 0;
        padding: 0;
    }
    h1 {
        font-size: 1.5rem;
    }
    table {
        margin-top: 2rem;
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
    table tr th {
        padding: 0.5rem 0.625rem 0.625rem;
        font-weight: bold;
        color: #444444;
    }
    table tr:nth-child(even) {
        background: #f6f6f6;
    }
    table tr:hover {
        background: #deeef7a6;
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
    th a, th a:visited, .chevron {
        color: #777c82;
    }
    .chevron {
        margin-right: .5rem;
        font-size: 1.2em;
        font-weight: bold;
    }
    th span.active a, th span.active span {
        color: #444444;
    }
    .back {
        position: fixed;
        bottom: 1.1rem;
        right: 0.625rem;
        background: #e0e0e0;
        border-radius: 100%;
        box-shadow: 0 0 8px -4px #888888;
        opacity: 0.8;
        padding: 1rem 1.1rem;
        color: #444444;
    }
    .back:visited {
        color: #444444;
    }
    .back:hover {
        color: #3498db;
        text-decoration: none;
    }
    .download {
        display: flex;
        flex-wrap: wrap;
        margin-top: .5rem;
        padding: 0.125rem;
    }
    .download a, .download a:visited {
        color: #3498db;
    }
    .download a {
        background: #efefef;
        padding: 0.5rem;
        border-radius: 0.2rem;
        margin-top: 1rem;
    }
    .download a:hover {
        background: #deeef7a6;
    }
    .download a:not(:last-of-type) {
        margin-right: 1rem;
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
            padding-bottom: 1rem;
        }
    }
    @media (max-width: 400px) {
        h1 {
            font-size: 1.375em;
        }
    }"#.to_string()))
}

/// Partial: up arrow
fn arrow_up() -> Markup {
    (PreEscaped("⇪".to_string()))
}

/// Partial: new line
fn br() -> Markup {
    (PreEscaped("<br>".to_string()))
}

/// Partial: chevron left
fn chevron_left() -> Markup {
    (PreEscaped("◂".to_string()))
}

/// Partial: chevron up
fn chevron_up() -> Markup {
    (PreEscaped("▴".to_string()))
}

/// Partial: chevron up
fn chevron_down() -> Markup {
    (PreEscaped("▾".to_string()))
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

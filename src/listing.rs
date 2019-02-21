use actix_web::{fs, HttpRequest, HttpResponse, Result};
use bytesize::ByteSize;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use clap::{_clap_count_exprs, arg_enum};
use htmlescape::encode_minimal as escape_html_entity;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use serde::Serialize;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::renderer;

arg_enum! {
    #[derive(Clone, Copy, Debug)]
    /// Available sorting methods
    ///
    /// Natural: natural sorting method
    /// 1 -> 2 -> 3 -> 11
    ///
    /// Alpha: pure alphabetical sorting method
    /// 1 -> 11 -> 2 -> 3
    ///
    /// DirsFirst: directories are listed first, alphabetical sorting is also applied
    /// 1/ -> 2/ -> 3/ -> 11 -> 12
    ///
    /// Date: sort by last modification date (most recent first)
    pub enum SortingMethods {
        Natural,
        Alpha,
        DirsFirst,
        Date
    }
}

/// Entry
#[derive(Debug, Serialize)]
pub struct Entry {
    /// Name of the entry
    name: String,

    /// Entry a directory
    is_dir: bool,

    /// URL of the entry
    link: String,

    /// Size in byte of the entry. Only available for files
    size: Option<String>,

    /// Last modification date
    last_modification_date: Option<SystemTime>,

    /// Last modification date-time (for display purposes)
    last_modification_datetime_str: (String, String),

    /// Last modification timer
    last_modification_timer: String,
}

impl Entry {
    fn new(
        name: String,
        is_dir: bool,
        link: String,
        size: Option<String>,
        last_modification_date: Option<SystemTime>,
        last_modification_datetime_str: (String, String),
        last_modification_timer: String,
    ) -> Self {
        Entry {
            name,
            is_dir,
            link,
            size,
            last_modification_date,
            last_modification_datetime_str,
            last_modification_timer,
        }
    }
}

pub fn file_handler(req: &HttpRequest<crate::MiniserveConfig>) -> Result<fs::NamedFile> {
    let path = &req.state().path;
    Ok(fs::NamedFile::open(path)?)
}

/// List a directory and renders a HTML file accordingly
/// Adapted from https://docs.rs/actix-web/0.7.13/src/actix_web/fs.rs.html#564
pub fn directory_listing<S>(
    dir: &fs::Directory,
    req: &HttpRequest<S>,
    skip_symlinks: bool,
    random_route: Option<String>,
    sort_method: SortingMethods,
    reverse_sort: bool,
) -> Result<HttpResponse, io::Error> {
    let renderer = renderer::Renderer::new()?;

    let title = format!("Index of {}", req.path());
    let mut is_root = true;
    let mut page_parent = None;

    let base = Path::new(req.path());
    let random_route = format!("/{}", random_route.unwrap_or_default());

    if let Some(parent) = base.parent() {
        if req.path() != random_route {
            is_root = false;
            page_parent = Some(parent.display().to_string());
        }
    }

    let mut entries: Vec<Entry> = Vec::new();

    for entry in dir.path.read_dir()? {
        if dir.is_visible(&entry) {
            let entry = entry.unwrap();
            let p = match entry.path().strip_prefix(&dir.path) {
                Ok(p) => base.join(p),
                Err(_) => continue,
            };
            // show file url as relative to static path
            let file_url =
                utf8_percent_encode(&p.to_string_lossy(), DEFAULT_ENCODE_SET).to_string();
            // " -- &quot;  & -- &amp;  ' -- &#x27;  < -- &lt;  > -- &gt;
            let file_name = escape_html_entity(&entry.file_name().to_string_lossy());

            // if file is a directory, add '/' to the end of the name
            if let Ok(metadata) = entry.metadata() {
                if skip_symlinks && metadata.file_type().is_symlink() {
                    continue;
                }
                let last_modification_date = match metadata.modified() {
                    Ok(date) => Some(date),
                    Err(_) => None,
                };

                if metadata.is_dir() {
                    entries.push(Entry::new(
                        file_name,
                        true,
                        file_url,
                        None,
                        last_modification_date,
                        convert_to_utc(last_modification_date),
                        humanize_systemtime(last_modification_date),
                    ));
                } else {
                    entries.push(Entry::new(
                        file_name,
                        false,
                        file_url,
                        Some(ByteSize::b(metadata.len()).to_string_as(false)),
                        last_modification_date,
                        convert_to_utc(last_modification_date),
                        humanize_systemtime(last_modification_date),
                    ));
                }
            } else {
                continue;
            }
        }
    }

    match sort_method {
        SortingMethods::Natural => entries
            .sort_by(|e1, e2| alphanumeric_sort::compare_str(e1.name.clone(), e2.name.clone())),
        SortingMethods::Alpha => {
            entries.sort_by(|e1, e2| e2.is_dir.partial_cmp(&e1.is_dir).unwrap());
            entries.sort_by_key(|e| e.name.clone())
        }
        SortingMethods::DirsFirst => {
            entries.sort_by_key(|e| e.name.clone());
            entries.sort_by(|e1, e2| e2.is_dir.partial_cmp(&e1.is_dir).unwrap());
        }
        SortingMethods::Date => entries.sort_by(|e1, e2| {
            // If, for some reason, we can't get the last modification date of an entry
            // let's consider it was modified on UNIX_EPOCH (01/01/19270 00:00:00)
            e2.last_modification_date
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .cmp(&e1.last_modification_date.unwrap_or(SystemTime::UNIX_EPOCH))
        }),
    };

    if reverse_sort {
        entries.reverse();
    }

    let template = renderer::PageTemplate::new(title, entries, is_root, page_parent);

    let body = renderer.render("index", template)?;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}

/// Converts a SystemTime object to a strings tuple (date, time)
/// Date is formatted as %e %b, e.g. Jul 12
/// Time is formatted as %R, e.g. 22:34
///
/// If no SystemTime was given, returns a tuple containing empty strings
fn convert_to_utc(src_time: Option<SystemTime>) -> (String, String) {
    src_time
        .map(|time| DateTime::<Utc>::from(time))
        .map(|date_time| {
            (
                date_time.format("%e %b").to_string(),
                date_time.format("%R").to_string(),
            )
        })
        .unwrap_or_default()
}

/// Converts a SystemTime to a string readable by a human,
/// i.e. calculates the duration between now() and the given SystemTime,
/// and gives a rough approximation of the elapsed time since
///
/// If no SystemTime was given, returns an empty string
fn humanize_systemtime(src_time: Option<SystemTime>) -> String {
    src_time
        .and_then(|std_time| SystemTime::now().duration_since(std_time).ok())
        .and_then(|from_now| Duration::from_std(from_now).ok())
        .map(|duration| HumanTime::from(duration).to_text_en(Accuracy::Rough, Tense::Past))
        .unwrap_or_default()
}

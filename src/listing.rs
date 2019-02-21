use actix_web::{fs, HttpRequest, HttpResponse, Result};
use clap::{_clap_count_exprs, arg_enum};
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use htmlescape::encode_minimal as escape_html_entity;
use bytesize::ByteSize;
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
#[derive(Debug)]
pub struct Entry {
    /// Name of the entry
    pub name: String,

    /// Entry is a directory
    pub is_dir: bool,

    /// URL of the entry
    pub link: String,

    /// Size in byte of the entry. Only available for files
    pub size: Option<bytesize::ByteSize>,

    /// Last modification date
    pub last_modification_date: Option<SystemTime>,
}

impl Entry {
    fn new(
        name: String,
        is_dir: bool,
        link: String,
        size: Option<bytesize::ByteSize>,
        last_modification_date: Option<SystemTime>,
    ) -> Self {
        Entry {
            name,
            is_dir,
            link,
            size,
            last_modification_date,
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
                    ));
                } else {
                    entries.push(Entry::new(
                        file_name,
                        false,
                        file_url,
                        Some(ByteSize::b(metadata.len())),
                        last_modification_date,
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

    let repr_entries = entries
        .into_iter()
        .map(|e| renderer::ReprEntry::from(e))
        .collect::<Vec<_>>();
    let template = renderer::PageTemplate::new(title, repr_entries, is_root, page_parent);

    let body = renderer.render("index", template)?;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}

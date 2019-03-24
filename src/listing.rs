use actix_web::{fs, http, Body, FromRequest, HttpRequest, HttpResponse, Query, Result};
use bytesize::ByteSize;
use futures::stream::once;
use htmlescape::encode_minimal as escape_html_entity;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use serde::Deserialize;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::archive;
use crate::errors;
use crate::renderer;

/// Query parameters
#[derive(Debug, Deserialize)]
struct QueryParameters {
    sort: Option<SortingMethod>,
    order: Option<SortingOrder>,
    download: Option<archive::CompressionMethod>,
}

/// Available sorting methods
#[derive(Debug, Deserialize, Clone)]
pub enum SortingMethod {
    /// Sort by name
    #[serde(alias = "name")]
    Name,

    /// Sort by size
    #[serde(alias = "size")]
    Size,

    /// Sort by last modification date (natural sort: follows alphanumerical order)
    #[serde(alias = "date")]
    Date,
}

impl SortingMethod {
    pub fn to_string(&self) -> String {
        match &self {
            SortingMethod::Name => "name",
            SortingMethod::Size => "size",
            SortingMethod::Date => "date",
        }
        .to_string()
    }
}

/// Available sorting orders
#[derive(Debug, Deserialize, Clone)]
pub enum SortingOrder {
    /// Ascending order
    #[serde(alias = "asc")]
    Ascending,

    /// Descending order
    #[serde(alias = "desc")]
    Descending,
}

impl SortingOrder {
    pub fn to_string(&self) -> String {
        match &self {
            SortingOrder::Ascending => "asc",
            SortingOrder::Descending => "desc",
        }
        .to_string()
    }
}

#[derive(PartialEq)]
/// Possible entry types
pub enum EntryType {
    /// Entry is a directory
    Directory,

    /// Entry is a file
    File,
}

/// Entry
pub struct Entry {
    /// Name of the entry
    pub name: String,

    /// Type of the entry
    pub entry_type: EntryType,

    /// URL of the entry
    pub link: String,

    /// Size in byte of the entry. Only available for EntryType::File
    pub size: Option<bytesize::ByteSize>,

    /// Last modification date
    pub last_modification_date: Option<SystemTime>,
}

impl Entry {
    fn new(
        name: String,
        entry_type: EntryType,
        link: String,
        size: Option<bytesize::ByteSize>,
        last_modification_date: Option<SystemTime>,
    ) -> Self {
        Entry {
            name,
            entry_type,
            link,
            size,
            last_modification_date,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Directory
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
) -> Result<HttpResponse, io::Error> {
    let title = format!("Index of {}", req.path());
    let base = Path::new(req.path());
    let random_route = format!("/{}", random_route.unwrap_or_default());
    let is_root = base.parent().is_none() || req.path() == random_route;
    let page_parent = base.parent().map(|p| p.display().to_string());

    let (sort_method, sort_order, download) =
        if let Ok(query) = Query::<QueryParameters>::extract(req) {
            (
                query.sort.clone(),
                query.order.clone(),
                query.download.clone(),
            )
        } else {
            (None, None, None)
        };

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
                        EntryType::Directory,
                        file_url,
                        None,
                        last_modification_date,
                    ));
                } else {
                    entries.push(Entry::new(
                        file_name,
                        EntryType::File,
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

    if let Some(sorting_method) = &sort_method {
        match sorting_method {
            SortingMethod::Name => entries
                .sort_by(|e1, e2| alphanumeric_sort::compare_str(e1.name.clone(), e2.name.clone())),
            SortingMethod::Size => entries.sort_by(|e1, e2| {
                // If we can't get the size of the entry (directory for instance)
                // let's consider it's 0b
                e2.size
                    .unwrap_or_else(|| ByteSize::b(0))
                    .cmp(&e1.size.unwrap_or_else(|| ByteSize::b(0)))
            }),
            SortingMethod::Date => entries.sort_by(|e1, e2| {
                // If, for some reason, we can't get the last modification date of an entry
                // let's consider it was modified on UNIX_EPOCH (01/01/19270 00:00:00)
                e2.last_modification_date
                    .unwrap_or(SystemTime::UNIX_EPOCH)
                    .cmp(&e1.last_modification_date.unwrap_or(SystemTime::UNIX_EPOCH))
            }),
        };
    } else {
        // Sort in alphanumeric order by default
        entries.sort_by(|e1, e2| alphanumeric_sort::compare_str(e1.name.clone(), e2.name.clone()))
    }

    if let Some(sorting_order) = &sort_order {
        if let SortingOrder::Descending = sorting_order {
            entries.reverse()
        }
    }

    if let Some(compression_method) = &download {
        log::info!(
            "Creating an archive ({extension}) of {path}...",
            extension = compression_method.extension(),
            path = &dir.path.display().to_string()
        );
        match archive::create_archive(&compression_method, &dir.path, skip_symlinks) {
            Ok((filename, content)) => {
                log::info!("{file} successfully created !", file = &filename);
                Ok(HttpResponse::Ok()
                    .content_type(compression_method.content_type())
                    .content_encoding(compression_method.content_encoding())
                    .header("Content-Transfer-Encoding", "binary")
                    .header(
                        "Content-Disposition",
                        format!("attachment; filename={:?}", filename),
                    )
                    .chunked()
                    .body(Body::Streaming(Box::new(once(Ok(content))))))
            }
            Err(err) => {
                errors::print_error_chain(err);
                Ok(HttpResponse::Ok()
                    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(""))
            }
        }
    } else {
        Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(
                renderer::page(
                    &title,
                    entries,
                    is_root,
                    page_parent,
                    sort_method,
                    sort_order,
                    &base.to_string_lossy(),
                )
                .into_string(),
            ))
    }
}

use actix_web::{fs, Body, FromRequest, HttpRequest, HttpResponse, Query, Result};
use bytesize::ByteSize;
use futures::Stream;
use htmlescape::encode_minimal as escape_html_entity;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use serde::Deserialize;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use strum_macros::{Display, EnumString};

use crate::archive::CompressionMethod;
use crate::errors::{self, ContextualError};
use crate::renderer;
use crate::themes::ColorScheme;

/// Query parameters
#[derive(Deserialize)]
pub struct QueryParameters {
    pub path: Option<PathBuf>,
    pub sort: Option<SortingMethod>,
    pub order: Option<SortingOrder>,
    pub theme: Option<ColorScheme>,
    download: Option<CompressionMethod>,
}

/// Available sorting methods
#[derive(Deserialize, Clone, EnumString, Display, Copy)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SortingMethod {
    /// Sort by name
    Name,

    /// Sort by size
    Size,

    /// Sort by last modification date (natural sort: follows alphanumerical order)
    Date,
}

/// Available sorting orders
#[derive(Deserialize, Clone, EnumString, Display, Copy)]
pub enum SortingOrder {
    /// Ascending order
    #[serde(alias = "asc")]
    #[strum(serialize = "asc")]
    Ascending,

    /// Descending order
    #[serde(alias = "desc")]
    #[strum(serialize = "desc")]
    Descending,
}

#[derive(PartialEq)]
/// Possible entry types
pub enum EntryType {
    /// Entry is a directory
    Directory,

    /// Entry is a file
    File,

    /// Entry is a symlink
    Symlink,
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

    /// Returns wether the entry is a directory
    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Directory
    }

    /// Returns wether the entry is a file
    pub fn is_file(&self) -> bool {
        self.entry_type == EntryType::File
    }

    /// Returns wether the entry is a symlink
    pub fn is_symlink(&self) -> bool {
        self.entry_type == EntryType::Symlink
    }
}

pub fn file_handler(req: &HttpRequest<crate::MiniserveConfig>) -> Result<fs::NamedFile> {
    let path = &req.state().path;
    Ok(fs::NamedFile::open(path)?)
}

/// List a directory and renders a HTML file accordingly
/// Adapted from https://docs.rs/actix-web/0.7.13/src/actix_web/fs.rs.html#564
#[allow(clippy::identity_conversion)]
pub fn directory_listing<S>(
    dir: &fs::Directory,
    req: &HttpRequest<S>,
    skip_symlinks: bool,
    file_upload: bool,
    random_route: Option<String>,
    default_color_scheme: ColorScheme,
    upload_route: String,
) -> Result<HttpResponse, io::Error> {
    let serve_path = req.path();
    let base = Path::new(serve_path);
    let random_route = format!("/{}", random_route.unwrap_or_default());
    let is_root = base.parent().is_none() || req.path() == random_route;
    let page_parent = base.parent().map(|p| p.display().to_string());
    let current_dir = match base.strip_prefix(random_route) {
        Ok(c_d) => Path::new("/").join(c_d),
        Err(_) => base.to_path_buf(),
    };

    let query_params = extract_query_parameters(req);

    let mut entries: Vec<Entry> = Vec::new();

    for entry in dir.path.read_dir()? {
        if dir.is_visible(&entry) {
            let entry = entry?;
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

                if metadata.file_type().is_symlink() {
                    entries.push(Entry::new(
                        file_name,
                        EntryType::Symlink,
                        file_url,
                        None,
                        last_modification_date,
                    ));
                } else if metadata.is_dir() {
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

    if let Some(sorting_method) = query_params.sort {
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

    if let Some(sorting_order) = query_params.order {
        if let SortingOrder::Descending = sorting_order {
            entries.reverse()
        }
    }

    let color_scheme = query_params.theme.unwrap_or(default_color_scheme);

    if let Some(compression_method) = query_params.download {
        log::info!(
            "Creating an archive ({extension}) of {path}...",
            extension = compression_method.extension(),
            path = &dir.path.display().to_string()
        );

        let filename = format!(
            "{}.{}",
            dir.path.file_name().unwrap().to_str().unwrap(),
            compression_method.extension()
        );

        // We will create the archive in a separate thread, and stream the content using a pipe.
        // The pipe is made of a futures channel, and an adapter to implement the `Write` trait.
        // Include 10 messages of buffer for erratic connection speeds.
        let (tx, rx) = futures::sync::mpsc::channel(10);
        let pipe = crate::pipe::Pipe::new(tx);

        // Start the actual archive creation in a separate thread.
        let dir = dir.path.to_path_buf();
        std::thread::spawn(move || {
            if let Err(err) = compression_method.create_archive(dir, skip_symlinks, pipe) {
                log::error!("Error during archive creation: {:?}", err);
            }
        });

        // `rx` is a receiver of bytes - it can act like a `Stream` of bytes, and that's exactly
        // what actix-web wants to stream the response.
        //
        // But right now the error types do not match:
        // `<rx as Stream>::Error == ()`, but we want `actix_web::error::Error`
        //
        // That being said, `rx` will never fail because the `Stream` implementation for `Receiver`
        // never returns an error - it simply cannot fail.
        let rx = rx.map_err(|_| unreachable!("pipes never fail"));

        // At this point, `rx` implements everything actix want for a streamed response,
        // so we can just give a `Box::new(rx)` as streaming body.

        Ok(HttpResponse::Ok()
            .content_type(compression_method.content_type())
            .content_encoding(compression_method.content_encoding())
            .header("Content-Transfer-Encoding", "binary")
            .header(
                "Content-Disposition",
                format!("attachment; filename={:?}", filename),
            )
            .chunked()
            .body(Body::Streaming(Box::new(rx))))
    } else {
        Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(
                renderer::page(
                    serve_path,
                    entries,
                    is_root,
                    page_parent,
                    query_params.sort,
                    query_params.order,
                    default_color_scheme,
                    color_scheme,
                    file_upload,
                    &upload_route,
                    &current_dir.display().to_string(),
                )
                .into_string(),
            ))
    }
}

pub fn extract_query_parameters<S>(req: &HttpRequest<S>) -> QueryParameters {
    match Query::<QueryParameters>::extract(req) {
        Ok(query) => QueryParameters {
            sort: query.sort,
            order: query.order,
            download: query.download,
            theme: query.theme,
            path: query.path.clone(),
        },
        Err(e) => {
            let err = ContextualError::ParseError("query parameters".to_string(), e.to_string());
            errors::log_error_chain(err.to_string());
            QueryParameters {
                sort: None,
                order: None,
                download: None,
                theme: None,
                path: None,
            }
        }
    }
}

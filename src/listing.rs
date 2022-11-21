#![allow(clippy::format_push_string)]
use std::collections::HashMap;
use std::fmt::Display;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use actix_web::{dev::ServiceResponse, web::Query, HttpMessage, HttpRequest, HttpResponse};
use bytesize::ByteSize;
use comrak::{markdown_to_html, ComrakOptions};
use log::warn;
use once_cell::sync::OnceCell;
use percent_encoding::{percent_decode_str, utf8_percent_encode};
use regex::Regex;
use serde::Deserialize;
use strum::{Display, EnumString};

use crate::archive::ArchiveMethod;
use crate::auth::CurrentUser;
use crate::errors::{self, ContextualError};
use crate::renderer;

use self::percent_encode_sets::PATH_SEGMENT;

static FILE_SIZE_CACHE: OnceCell<Arc<Mutex<HashMap<PathBuf, u64>>>> = OnceCell::new();

/// "percent-encode sets" as defined by WHATWG specs:
/// https://url.spec.whatwg.org/#percent-encoded-bytes
mod percent_encode_sets {
    use percent_encoding::{AsciiSet, CONTROLS};
    const BASE: &AsciiSet = &CONTROLS.add(b'%');
    pub const QUERY: &AsciiSet = &BASE.add(b' ').add(b'"').add(b'#').add(b'<').add(b'>');
    pub const PATH: &AsciiSet = &QUERY.add(b'?').add(b'`').add(b'{').add(b'}');
    pub const PATH_SEGMENT: &AsciiSet = &PATH.add(b'/').add(b'\\');
}

/// Query parameters
#[derive(Deserialize, Default)]
pub struct QueryParameters {
    pub path: Option<PathBuf>,
    pub sort: Option<SortingMethod>,
    pub order: Option<SortingOrder>,
    pub raw: Option<bool>,
    pub mkdir_name: Option<String>,
    download: Option<ArchiveMethod>,
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

#[derive(PartialEq, Eq)]
/// Possible entry types
pub enum EntryType {
    /// Entry is a directory
    Directory,

    /// Entry is a file
    File,
}

#[derive(Clone, Copy, PartialEq, Eq)]
/// Possible entry size types
pub enum EntrySize {
    /// EntryCount is number of entries in a directory
    EntryCount(usize),
    /// Bytes is number of bytes in a file
    Bytes(bytesize::ByteSize),
}

impl Display for EntrySize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntrySize::EntryCount(count) => write!(f, "{}", count),
            EntrySize::Bytes(bytes) => write!(f, "{}", bytes),
        }
    }
}

/// Entry
pub struct Entry {
    /// Name of the entry
    pub name: String,

    /// Type of the entry
    pub entry_type: EntryType,

    /// URL of the entry
    pub link: String,

    /// Size of the entry
    pub size: EntrySize,

    /// Last modification date
    pub last_modification_date: Option<SystemTime>,

    /// Path of symlink pointed to
    pub symlink_info: Option<String>,
}

impl Entry {
    fn new(
        name: String,
        entry_type: EntryType,
        link: String,
        size: EntrySize,
        last_modification_date: Option<SystemTime>,
        symlink_info: Option<String>,
    ) -> Self {
        Entry {
            name,
            entry_type,
            link,
            size,
            last_modification_date,
            symlink_info,
        }
    }

    /// Returns whether the entry is a directory
    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Directory
    }

    /// Returns whether the entry is a file
    pub fn is_file(&self) -> bool {
        self.entry_type == EntryType::File
    }
}

/// One entry in the path to the listed directory
pub struct Breadcrumb {
    /// Name of directory
    pub name: String,

    /// Link to get to directory, relative to listed directory
    pub link: String,
}

impl Breadcrumb {
    fn new(name: String, link: String) -> Self {
        Breadcrumb { name, link }
    }
}

pub async fn file_handler(req: HttpRequest) -> actix_web::Result<actix_files::NamedFile> {
    let path = &req.app_data::<crate::MiniserveConfig>().unwrap().path;
    actix_files::NamedFile::open(path).map_err(Into::into)
}

/// List a directory and renders a HTML file accordingly
/// Adapted from https://docs.rs/actix-web/0.7.13/src/actix_web/fs.rs.html#564
pub fn directory_listing(
    dir: &actix_files::Directory,
    req: &HttpRequest,
) -> io::Result<ServiceResponse> {
    let extensions = req.extensions();
    let current_user: Option<&CurrentUser> = extensions.get::<CurrentUser>();

    let conf = req.app_data::<crate::MiniserveConfig>().unwrap();
    let serve_path = req.path();

    let base = Path::new(serve_path);
    let random_route_abs = format!("/{}", conf.route_prefix);
    let abs_url = format!(
        "{}://{}{}",
        req.connection_info().scheme(),
        req.connection_info().host(),
        req.uri()
    );
    let is_root = base.parent().is_none() || Path::new(&req.path()) == Path::new(&random_route_abs);

    let encoded_dir = match base.strip_prefix(random_route_abs) {
        Ok(c_d) => Path::new("/").join(c_d),
        Err(_) => base.to_path_buf(),
    }
    .display()
    .to_string();

    let breadcrumbs = {
        let title = conf
            .title
            .clone()
            .unwrap_or_else(|| req.connection_info().host().into());

        let decoded = percent_decode_str(&encoded_dir).decode_utf8_lossy();

        let mut res: Vec<Breadcrumb> = Vec::new();

        let mut link_accumulator = format!("{}/", &conf.route_prefix);

        let mut components = Path::new(&*decoded).components().peekable();

        while let Some(c) = components.next() {
            let name;

            match c {
                Component::RootDir => {
                    name = title.clone();
                }
                Component::Normal(s) => {
                    name = s.to_string_lossy().to_string();
                    link_accumulator
                        .push_str(&(utf8_percent_encode(&name, PATH_SEGMENT).to_string() + "/"));
                }
                _ => name = "".to_string(),
            };

            res.push(Breadcrumb::new(
                name,
                if components.peek().is_some() {
                    link_accumulator.clone()
                } else {
                    ".".to_string()
                },
            ));
        }
        res
    };

    let query_params = extract_query_parameters(req);

    let mut entries: Vec<Entry> = Vec::new();
    let mut readme: Option<(String, String)> = None;
    let readme_rx: Regex = Regex::new("^readme([.](md|txt))?$").unwrap();

    for entry in dir.path.read_dir()? {
        if dir.is_visible(&entry) || conf.show_hidden {
            let entry = entry?;
            // show file url as relative to static path
            let file_name = entry.file_name().to_string_lossy().to_string();
            let (is_symlink, metadata) = match entry.metadata() {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    // for symlinks, get the metadata of the original file
                    (true, std::fs::metadata(entry.path()))
                }
                res => (false, res),
            };
            let symlink_dest = (is_symlink && conf.show_symlink_info)
                .then(|| entry.path())
                .and_then(|path| std::fs::read_link(path).ok())
                .map(|path| path.to_string_lossy().into_owned());
            let file_url = base
                .join(&utf8_percent_encode(&file_name, PATH_SEGMENT).to_string())
                .to_string_lossy()
                .to_string();

            // if file is a directory, add '/' to the end of the name
            if let Ok(metadata) = metadata {
                if conf.no_symlinks && is_symlink {
                    continue;
                }
                let last_modification_date = match metadata.modified() {
                    Ok(date) => Some(date),
                    Err(_) => None,
                };

                let cache_map = FILE_SIZE_CACHE
                    .get_or_init(|| {
                        println!("init cache_map");
                        Arc::default()
                    })
                    .clone();
                let size = {
                    match fs_extra::dir::get_size(&entry.path()) {
                        Err(_) => EntrySize::EntryCount({
                            std::fs::read_dir(entry.path())
                                .into_iter()
                                .take(500_000)
                                .count()
                        }),
                        Ok(result) => {
                            if let Ok(mut lock) = cache_map.lock() {
                                lock.insert(entry.path().to_path_buf(), result);
                            } else {
                                warn!("Failed to write to cache");
                            };
                            EntrySize::Bytes(ByteSize::b(result))
                        }
                    }
                };
                if metadata.is_dir() {
                    entries.push(Entry::new(
                        file_name,
                        EntryType::Directory,
                        file_url,
                        size,
                        last_modification_date,
                        symlink_dest,
                    ));
                } else if metadata.is_file() {
                    entries.push(Entry::new(
                        file_name.clone(),
                        EntryType::File,
                        file_url,
                        EntrySize::Bytes(ByteSize::b(metadata.len())),
                        last_modification_date,
                        symlink_dest,
                    ));
                    if conf.readme && readme_rx.is_match(&file_name.to_lowercase()) {
                        let ext = file_name.split('.').last().unwrap().to_lowercase();
                        readme = Some((
                            file_name.to_string(),
                            if ext == "md" {
                                markdown_to_html(
                                    &std::fs::read_to_string(entry.path())?,
                                    &ComrakOptions::default(),
                                )
                            } else {
                                format!("<pre>{}</pre>", &std::fs::read_to_string(entry.path())?)
                            },
                        ));
                    }
                }
            } else {
                continue;
            }
        }
    }

    match query_params.sort.unwrap_or(SortingMethod::Name) {
        SortingMethod::Name => entries.sort_by(|e1, e2| {
            alphanumeric_sort::compare_str(e1.name.to_lowercase(), e2.name.to_lowercase())
        }),
        SortingMethod::Size => entries.sort_by(|e1, e2| {
            // If we can't get the size of the entry (directory for instance)
            // let's consider it's 0b
            match (e1.size, e2.size) {
                (EntrySize::EntryCount(ref e1_count), EntrySize::EntryCount(ref e2_count)) => {
                    e2_count.cmp(e1_count)
                }
                (EntrySize::Bytes(ref e1_bytes), EntrySize::Bytes(ref e2_bytes)) => {
                    e2_bytes.cmp(e1_bytes)
                }
                (EntrySize::EntryCount(_), EntrySize::Bytes(_)) => std::cmp::Ordering::Greater,
                (EntrySize::Bytes(_), EntrySize::EntryCount(_)) => std::cmp::Ordering::Less,
            }
            // e2.size
            //     .unwrap_or_else(|| ByteSize::b(0))
            //     .cmp(&e1.size.unwrap_or_else(|| ByteSize::b(0)))
        }),
        SortingMethod::Date => entries.sort_by(|e1, e2| {
            // If, for some reason, we can't get the last modification date of an entry
            // let's consider it was modified on UNIX_EPOCH (01/01/19270 00:00:00)
            e2.last_modification_date
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .cmp(&e1.last_modification_date.unwrap_or(SystemTime::UNIX_EPOCH))
        }),
    };

    if let Some(SortingOrder::Descending) = query_params.order {
        entries.reverse()
    }

    // List directories first
    if conf.dirs_first {
        entries.sort_by_key(|e| !e.is_dir());
    }

    if let Some(archive_method) = query_params.download {
        if !archive_method.is_enabled(conf.tar_enabled, conf.tar_gz_enabled, conf.zip_enabled) {
            return Ok(ServiceResponse::new(
                req.clone(),
                HttpResponse::Forbidden()
                    .content_type(mime::TEXT_PLAIN_UTF_8)
                    .body("Archive creation is disabled."),
            ));
        }
        log::info!(
            "Creating an archive ({extension}) of {path}...",
            extension = archive_method.extension(),
            path = &dir.path.display().to_string()
        );

        let file_name = format!(
            "{}.{}",
            dir.path.file_name().unwrap().to_str().unwrap(),
            archive_method.extension()
        );

        // We will create the archive in a separate thread, and stream the content using a pipe.
        // The pipe is made of a futures channel, and an adapter to implement the `Write` trait.
        // Include 10 messages of buffer for erratic connection speeds.
        let (tx, rx) = futures::channel::mpsc::channel::<io::Result<actix_web::web::Bytes>>(10);
        let pipe = crate::pipe::Pipe::new(tx);

        // Start the actual archive creation in a separate thread.
        let dir = dir.path.to_path_buf();
        let skip_symlinks = conf.no_symlinks;
        std::thread::spawn(move || {
            if let Err(err) = archive_method.create_archive(dir, skip_symlinks, pipe) {
                log::error!("Error during archive creation: {:?}", err);
            }
        });

        Ok(ServiceResponse::new(
            req.clone(),
            HttpResponse::Ok()
                .content_type(archive_method.content_type())
                .append_header(archive_method.content_encoding())
                .append_header(("Content-Transfer-Encoding", "binary"))
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename={:?}", file_name),
                ))
                .body(actix_web::body::BodyStream::new(rx)),
        ))
    } else {
        Ok(ServiceResponse::new(
            req.clone(),
            HttpResponse::Ok().content_type(mime::TEXT_HTML_UTF_8).body(
                renderer::page(
                    entries,
                    readme,
                    abs_url,
                    is_root,
                    query_params,
                    &breadcrumbs,
                    &encoded_dir,
                    conf,
                    current_user,
                )
                .into_string(),
            ),
        ))
    }
}

pub fn extract_query_parameters(req: &HttpRequest) -> QueryParameters {
    match Query::<QueryParameters>::from_query(req.query_string()) {
        Ok(Query(query_params)) => query_params,
        Err(e) => {
            let err = ContextualError::ParseError("query parameters".to_string(), e.to_string());
            errors::log_error_chain(err.to_string());
            QueryParameters::default()
        }
    }
}

use actix_web::{fs, HttpRequest, HttpResponse, Result};
use bytesize::ByteSize;
use htmlescape::encode_minimal as escape_html_entity;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use std::cmp::Ordering;
use std::fmt::Write as FmtWrite;
use std::io;
use std::path::Path;

arg_enum! {
    #[derive(Clone, Copy, Debug)]
    /// Available sorting methods
    pub enum SortingMethods {
        Natural,
        Alpha,
        DirsFirst,
    }
}

#[derive(PartialEq)]
/// Possible entry types
enum EntryType {
    /// Entry is a directory
    Directory,

    /// Entry is a file
    File,
}

impl PartialOrd for EntryType {
    fn partial_cmp(&self, other: &EntryType) -> Option<Ordering> {
        match (self, other) {
            (EntryType::Directory, EntryType::File) => Some(Ordering::Less),
            (EntryType::File, EntryType::Directory) => Some(Ordering::Greater),
            _ => Some(Ordering::Equal),
        }
    }
}

/// Entry
struct Entry {
    /// Name of the entry
    name: String,

    /// Type of the entry
    entry_type: EntryType,

    /// URL of the entry
    link: String,

    /// Size in byte of the entry. Only available for EntryType::File
    size: Option<bytesize::ByteSize>,
}

impl Entry {
    fn new(
        name: String,
        entry_type: EntryType,
        link: String,
        size: Option<bytesize::ByteSize>,
    ) -> Self {
        Entry {
            name,
            entry_type,
            link,
            size,
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
    let index_of = format!("Index of {}", req.path());
    let mut body = String::new();
    let base = Path::new(req.path());
    let random_route = format!("/{}", random_route.unwrap_or_default());

    if let Some(parent) = base.parent() {
        if req.path() != random_route {
            let _ = write!(
                body,
                "<tr><td><a class=\"root\" href=\"{}\">..</a></td><td></td></tr>",
                parent.display()
            );
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
                if metadata.is_dir() {
                    entries.push(Entry::new(file_name, EntryType::Directory, file_url, None));
                } else {
                    entries.push(Entry::new(
                        file_name,
                        EntryType::File,
                        file_url,
                        Some(ByteSize::b(metadata.len())),
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
            entries.sort_by(|e1, e2| e1.entry_type.partial_cmp(&e2.entry_type).unwrap());
            entries.sort_by_key(|e| e.name.clone())
        }
        SortingMethods::DirsFirst => {
            entries.sort_by_key(|e| e.name.clone());
            entries.sort_by(|e1, e2| e1.entry_type.partial_cmp(&e2.entry_type).unwrap());
        }
    };

    if reverse_sort {
        entries.reverse();
    }

    for entry in entries {
        match entry.entry_type {
            EntryType::Directory => {
                let _ = write!(
                    body,
                    "<tr><td><a class=\"directory\" href=\"{}\">{}/</a></td><td></td></tr>",
                    entry.link, entry.name
                );
            }
            EntryType::File => {
                let _ = write!(
                    body,
                    "<tr><td><a class=\"file\" href=\"{}\">{}</a></td><td>{}</td></tr>",
                    entry.link,
                    entry.name,
                    entry.size.unwrap()
                );
            }
        }
    }

    let html = format!(
        "<html>\
         <head>\
         <title>{}</title>\
         <style>\
         body {{\
           margin: 0;\
           font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto,\"Helvetica Neue\", Helvetica, Arial, sans-serif;\
           font-weight: 300;\
           color: #444444;\
           padding: 0.125rem;\
         }}\
         table {{\
           width: 100%;\
           background: white;\
           border: 0;\
           table-layout: auto;\
         }}\
         table thead {{\
           background: #efefef;\
         }}\
         table tr th,\
         table tr td {{\
           padding: 0.5625rem 0.625rem;\
           font-size: 0.875rem;\
           color: #777c82;\
           text-align: left;\
           line-height: 1.125rem;\
         }}\
         table thead tr th {{\
           padding: 0.5rem 0.625rem 0.625rem;\
           font-weight: bold;\
           color: #444444;\
         }}\
         table tr:nth-child(even) {{\
           background: #f6f6f6;\
         }}\
         a {{\
           text-decoration: none;\
           color: #3498db;\
         }}\
         a.root, a.root:visited {{\
            font-weight: bold;\
            color: #777c82;\
         }}\
         a.directory {{\
           font-weight: bold;\
         }}\
         a:hover {{\
           text-decoration: underline;\
         }}\
         a:visited {{\
           color: #8e44ad;\
         }}\
         @media (max-width: 600px) {{\
           h1 {{\
              font-size: 1.375em;\
           }}\
         }}\
         @media (max-width: 400px) {{\
           h1 {{\
              font-size: 1.375em;\
           }}\
         }}\
         </style>\
         </head>\
         <body><h1>{}</h1>\
         <table>\
         <thead><th>Name</th><th>Size</th></thead>\
         <tbody>\
         {}\
         </tbody></table></body>\n</html>",
        index_of, index_of, body
    );
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

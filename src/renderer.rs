use chrono::{DateTime, Duration, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use handlebars::Handlebars;
use serde::Serialize;
use std::io;
use std::time::SystemTime;

use crate::listing;

/// Representation of an entry, for display
#[derive(Debug, Serialize)]
pub struct ReprEntry {
    /// Name of the entry
    name: String,

    /// Entry a directory
    is_dir: bool,

    /// URL of the entry
    link: String,

    /// Size in byte of the entry. Only available for files
    size: Option<String>,

    /// Last modification date-time (for display purposes)
    last_modification_datetime: (String, String),

    /// Last modification timer
    last_modification_timer: String,
}

/// Converts a listing::Entry into a ReprEntry
impl From<listing::Entry> for ReprEntry {
    fn from(entry: listing::Entry) -> Self {
        let size = entry.size.map(|s| s.to_string_as(false));

        ReprEntry {
            name: entry.name,
            is_dir: entry.is_dir,
            link: entry.link,
            size,
            last_modification_datetime: convert_to_utc(entry.last_modification_date),
            last_modification_timer: humanize_systemtime(entry.last_modification_date),
        }
    }
}

/// Page template
#[derive(Debug, Serialize)]
pub struct PageTemplate {
    pub title: String,
    pub entries: Vec<ReprEntry>,
    pub is_root: bool,
    pub parent: Option<String>,
}

impl PageTemplate {
    pub fn new(
        title: String,
        entries: Vec<ReprEntry>,
        is_root: bool,
        parent: Option<String>,
    ) -> Self {
        PageTemplate {
            title,
            entries,
            is_root,
            parent,
        }
    }
}

/// Handlebar renderer
#[derive(Debug)]
pub struct Renderer {
    handlebars: Handlebars,
}

impl Renderer {
    /// Creates an handlebar instance
    pub fn new() -> Result<Self, io::Error> {
        let handlebars = Renderer::init_renderer()?;
        Ok(Renderer { handlebars })
    }

    /// Registers index.html as template
    fn init_renderer() -> Result<Handlebars, io::Error> {
        let mut handlebars = Handlebars::new();
        handlebars
            .register_template_file("index", "./src/templates/index.html")
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to register template {}", e),
                )
            })?;
        Ok(handlebars)
    }

    /// Renders data in the index template
    pub fn render<T>(&self, data: T) -> Result<String, io::Error>
    where
        T: Serialize + std::fmt::Debug,
    {
        self.handlebars
            .render("index", &data)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    }
}

/// Converts a SystemTime object to a strings tuple (date, time)
/// Date is formatted as %e %b, e.g. Jul 12
/// Time is formatted as %R, e.g. 22:34
///
/// If no SystemTime was given, returns a tuple containing empty strings
fn convert_to_utc(src_time: Option<SystemTime>) -> (String, String) {
    src_time
        .map(DateTime::<Utc>::from)
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

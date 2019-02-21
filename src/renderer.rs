use handlebars::Handlebars;
use serde::Serialize;
use std::io;

use crate::listing;

/// Page template
#[derive(Debug, Serialize)]
pub struct PageTemplate {
    pub title: String,
    pub entries: Vec<listing::Entry>,
    pub is_root: bool,
    pub parent: Option<String>,
}

impl PageTemplate {
    pub fn new(
        title: String,
        entries: Vec<listing::Entry>,
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

#[derive(Debug)]
pub struct Renderer {
    handlebars: Handlebars,
}

impl Renderer {
    pub fn new() -> Result<Self, io::Error> {
        let handlebars = Renderer::init_renderer()?;
        Ok(Renderer { handlebars })
    }

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

    pub fn render<T>(&self, template_name: &str, data: T) -> Result<String, io::Error>
    where
        T: Serialize + std::fmt::Debug,
    {
        self.handlebars
            .render(&template_name, &data)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e.to_string())))
    }
}

use actix_web::{fs, App, HttpRequest, Result};
use std::net::IpAddr;

use crate::auth;
use crate::listing;

#[derive(Clone, Debug)]
pub struct MiniserveConfig {
    pub verbose: bool,
    pub path: std::path::PathBuf,
    pub port: u16,
    pub interfaces: Vec<IpAddr>,
    pub auth: Option<auth::BasicAuthParams>,
    pub path_explicitly_chosen: bool,
    pub no_symlinks: bool,
    pub random_route: Option<String>,
    pub sort_method: listing::SortingMethods,
    pub reverse_sort: bool,
}

pub fn configure_app(app: App<MiniserveConfig>) -> App<MiniserveConfig> {
    let s = {
        let path = &app.state().path;
        let no_symlinks = app.state().no_symlinks;
        let random_route = app.state().random_route.clone();
        let sort_method = app.state().sort_method.clone();
        let reverse_sort = app.state().reverse_sort;
        if path.is_file() {
            None
        } else {
            Some(
                fs::StaticFiles::new(path)
                    .expect("Couldn't create path")
                    .show_files_listing()
                    .files_listing_renderer(move |dir, req| {
                        listing::directory_listing(
                            dir,
                            req,
                            no_symlinks,
                            random_route.clone(),
                            sort_method.clone(),
                            reverse_sort,
                        )
                    }),
            )
        }
    };

    let random_route = app.state().random_route.clone().unwrap_or_default();
    let full_route = format!("/{}", random_route);

    if let Some(s) = s {
        app.handler(&full_route, s)
    } else {
        app.resource(&full_route, |r| r.f(file_handler))
    }
}

fn file_handler(req: &HttpRequest<MiniserveConfig>) -> Result<fs::NamedFile> {
    let path = &req.state().path;
    Ok(fs::NamedFile::open(path)?)
}

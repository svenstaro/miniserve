use actix_web::web;
use actix_web::{
    http::{header::ContentType, StatusCode},
    Responder,
};
use actix_web::{middleware, App, HttpRequest, HttpResponse};
use actix_web_httpauth::middleware::HttpAuthentication;
use http::header::HeaderMap;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;
use std::time::Duration;
use structopt::clap::crate_version;
use yansi::{Color, Paint};

mod archive;
mod args;
mod auth;
mod errors;
mod file_upload;
mod listing;
mod pipe;
mod renderer;

use crate::errors::ContextualError;

#[derive(Clone)]
/// Configuration of the Miniserve application
pub struct MiniserveConfig {
    /// Enable verbose mode
    pub verbose: bool,

    /// Path to be served by miniserve
    pub path: std::path::PathBuf,

    /// Port on which miniserve will be listening
    pub port: u16,

    /// IP address(es) on which miniserve will be available
    pub interfaces: Vec<IpAddr>,

    /// Enable HTTP basic authentication
    pub auth: Vec<auth::RequiredAuth>,

    /// If false, miniserve will serve the current working directory
    pub path_explicitly_chosen: bool,

    /// Enable symlink resolution
    pub no_symlinks: bool,

    /// Show hidden files
    pub show_hidden: bool,

    /// Enable random route generation
    pub random_route: Option<String>,

    /// Randomly generated favicon route
    pub favicon_route: String,

    /// Randomly generated css route
    pub css_route: String,

    /// Default color scheme
    pub default_color_scheme: String,

    /// Default dark mode color scheme
    pub default_color_scheme_dark: String,

    /// The name of a directory index file to serve, like "index.html"
    ///
    /// Normally, when miniserve serves a directory, it creates a listing for that directory.
    /// However, if a directory contains this file, miniserve will serve that file instead.
    pub index: Option<std::path::PathBuf>,

    /// Enable QR code display
    pub show_qrcode: bool,

    /// Enable file upload
    pub file_upload: bool,

    /// Enable upload to override existing files
    pub overwrite_files: bool,

    /// If false, creation of tar archives is disabled
    pub tar_enabled: bool,

    /// If false, creation of zip archives is disabled
    pub zip_enabled: bool,

    /// If enabled, directories are listed first
    pub dirs_first: bool,

    /// Shown instead of host in page title and heading
    pub title: Option<String>,

    /// If specified, header will be added
    pub header: Vec<HeaderMap>,

    /// If enabled, version footer is hidden
    pub hide_version_footer: bool,
}

fn main() {
    match run() {
        Ok(()) => (),
        Err(e) => errors::log_error_chain(e.to_string()),
    }
}

#[actix_web::main(miniserve)]
async fn run() -> Result<(), ContextualError> {
    if cfg!(windows) && !Paint::enable_windows_ascii() {
        Paint::disable();
    }

    let miniserve_config = args::parse_args();

    let log_level = if miniserve_config.verbose {
        simplelog::LevelFilter::Info
    } else {
        simplelog::LevelFilter::Error
    };

    if simplelog::TermLogger::init(
        log_level,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
    )
    .is_err()
    {
        simplelog::SimpleLogger::init(log_level, simplelog::Config::default())
            .expect("Couldn't initialize logger")
    }

    if miniserve_config.no_symlinks {
        let is_symlink = miniserve_config
            .path
            .symlink_metadata()
            .map_err(|e| {
                ContextualError::IoError("Failed to retrieve symlink's metadata".to_string(), e)
            })?
            .file_type()
            .is_symlink();

        if is_symlink {
            return Err(ContextualError::from(
                "The no-symlinks option cannot be used with a symlink path".to_string(),
            ));
        }
    }

    let inside_config = miniserve_config.clone();

    let interfaces = miniserve_config
        .interfaces
        .iter()
        .map(|&interface| {
            if interface == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
                // If the interface is 0.0.0.0, we'll change it to 127.0.0.1 so that clicking the link will
                // also work on Windows. Why can't Windows interpret 0.0.0.0?
                "127.0.0.1".to_string()
            } else if interface.is_ipv6() {
                // If the interface is IPv6 then we'll print it with brackets so that it is clickable.
                format!("[{}]", interface)
            } else {
                format!("{}", interface)
            }
        })
        .collect::<Vec<String>>();

    let canon_path = miniserve_config.path.canonicalize().map_err(|e| {
        ContextualError::IoError("Failed to resolve path to be served".to_string(), e)
    })?;

    if let Some(index_path) = &miniserve_config.index {
        let has_index: std::path::PathBuf = [&canon_path, index_path].iter().collect();
        if !has_index.exists() {
            println!(
                "{warning} The provided index file could not be found.",
                warning = Color::RGB(255, 192, 0).paint("Notice:").bold()
            );
        }
    }
    let path_string = canon_path.to_string_lossy();

    println!(
        "{name} v{version}",
        name = Paint::new("miniserve").bold(),
        version = crate_version!()
    );
    if !miniserve_config.path_explicitly_chosen {
        println!("{warning} miniserve has been invoked without an explicit path so it will serve the current directory.", warning=Color::RGB(255, 192, 0).paint("Notice:").bold());
        println!(
            "      Invoke with -h|--help to see options or invoke as `miniserve .` to hide this advice."
        );
        print!("Starting server in ");
        io::stdout()
            .flush()
            .map_err(|e| ContextualError::IoError("Failed to write data".to_string(), e))?;
        for c in "3… 2… 1… \n".chars() {
            print!("{}", c);
            io::stdout()
                .flush()
                .map_err(|e| ContextualError::IoError("Failed to write data".to_string(), e))?;
            thread::sleep(Duration::from_millis(500));
        }
    }
    let mut addresses = String::new();
    for interface in &interfaces {
        if !addresses.is_empty() {
            addresses.push_str(", ");
        }
        addresses.push_str(&format!(
            "{}",
            Color::Green
                .paint(format!(
                    "http://{interface}:{port}",
                    interface = &interface,
                    port = miniserve_config.port
                ))
                .bold()
        ));

        if let Some(random_route) = miniserve_config.clone().random_route {
            addresses.push_str(&format!(
                "{}",
                Color::Green
                    .paint(format!("/{random_route}", random_route = random_route,))
                    .bold()
            ));
        }
    }

    let socket_addresses = interfaces
        .iter()
        .map(|interface| {
            format!(
                "{interface}:{port}",
                interface = &interface,
                port = miniserve_config.port,
            )
            .parse::<SocketAddr>()
        })
        .collect::<Result<Vec<SocketAddr>, _>>();

    let socket_addresses = match socket_addresses {
        Ok(addresses) => addresses,
        Err(e) => {
            // Note that this should never fail, since CLI parsing succeeded
            // This means the format of each IP address is valid, and so is the port
            // Valid IpAddr + valid port == valid SocketAddr
            return Err(ContextualError::ParseError(
                "string as socket address".to_string(),
                e.to_string(),
            ));
        }
    };

    let srv = actix_web::HttpServer::new(move || {
        App::new()
            .wrap(configure_header(&inside_config.clone()))
            .app_data(inside_config.clone())
            .wrap(middleware::Condition::new(
                !inside_config.auth.is_empty(),
                HttpAuthentication::basic(auth::handle_auth),
            ))
            .wrap(middleware::Logger::default())
            .route(
                &format!("/{}", inside_config.favicon_route),
                web::get().to(favicon),
            )
            .route(&format!("/{}", inside_config.css_route), web::get().to(css))
            .configure(|c| configure_app(c, &inside_config))
            .default_service(web::get().to(error_404))
    })
    .bind(socket_addresses.as_slice())
    .map_err(|e| ContextualError::IoError("Failed to bind server".to_string(), e))?
    .shutdown_timeout(0)
    .run();

    println!(
        "Serving path {path} at {addresses}",
        path = Color::Yellow.paint(path_string).bold(),
        addresses = addresses,
    );

    println!("\nQuit by pressing CTRL-C");

    srv.await
        .map_err(|e| ContextualError::IoError("".to_owned(), e))
}

fn configure_header(conf: &MiniserveConfig) -> middleware::DefaultHeaders {
    let headers = conf.clone().header;

    let mut default_headers = middleware::DefaultHeaders::new();
    for header in headers {
        for (header_name, header_value) in header.into_iter() {
            if let Some(header_name) = header_name {
                default_headers = default_headers.header(&header_name, header_value);
            }
        }
    }
    default_headers
}

/// Configures the Actix application
fn configure_app(app: &mut web::ServiceConfig, conf: &MiniserveConfig) {
    let random_route = conf.random_route.clone().unwrap_or_default();
    let uses_random_route = conf.random_route.clone().is_some();
    let full_route = format!("/{}", random_route);

    let upload_route;
    let serve_path = {
        let path = &conf.path;
        let no_symlinks = conf.no_symlinks;
        let show_hidden = conf.show_hidden;
        let random_route = conf.random_route.clone();
        let favicon_route = conf.favicon_route.clone();
        let css_route = conf.css_route.clone();
        let default_color_scheme = conf.default_color_scheme.clone();
        let default_color_scheme_dark = conf.default_color_scheme_dark.clone();
        let show_qrcode = conf.show_qrcode;
        let file_upload = conf.file_upload;
        let tar_enabled = conf.tar_enabled;
        let zip_enabled = conf.zip_enabled;
        let dirs_first = conf.dirs_first;
        let hide_version_footer = conf.hide_version_footer;
        let title = conf.title.clone();
        upload_route = if let Some(random_route) = conf.random_route.clone() {
            format!("/{}/upload", random_route)
        } else {
            "/upload".to_string()
        };
        if path.is_file() {
            None
        } else if let Some(index_file) = &conf.index {
            Some(
                actix_files::Files::new(&full_route, path).index_file(index_file.to_string_lossy()),
            )
        } else {
            let u_r = upload_route.clone();
            let files;
            if show_hidden {
                files = actix_files::Files::new(&full_route, path)
                    .show_files_listing()
                    .use_hidden_files();
            } else {
                files = actix_files::Files::new(&full_route, path).show_files_listing();
            }

            let files = files
                .files_listing_renderer(move |dir, req| {
                    listing::directory_listing(
                        dir,
                        req,
                        no_symlinks,
                        show_hidden,
                        file_upload,
                        random_route.clone(),
                        favicon_route.clone(),
                        css_route.clone(),
                        &default_color_scheme,
                        &default_color_scheme_dark,
                        show_qrcode,
                        u_r.clone(),
                        tar_enabled,
                        zip_enabled,
                        dirs_first,
                        hide_version_footer,
                        title.clone(),
                    )
                })
                .default_handler(web::to(error_404));
            Some(files)
        }
    };

    let favicon_route = conf.favicon_route.clone();
    let css_route = conf.css_route.clone();

    let default_color_scheme = conf.default_color_scheme.clone();
    let default_color_scheme_dark = conf.default_color_scheme_dark.clone();
    let hide_version_footer = conf.hide_version_footer;

    if let Some(serve_path) = serve_path {
        if conf.file_upload {
            // Allow file upload
            app.service(
                web::resource(&upload_route).route(web::post().to(move |req, payload| {
                    file_upload::upload_file(
                        req,
                        payload,
                        uses_random_route,
                        favicon_route.clone(),
                        css_route.clone(),
                        &default_color_scheme,
                        &default_color_scheme_dark,
                        hide_version_footer,
                    )
                })),
            )
            // Handle directories
            .service(serve_path);
        } else {
            // Handle directories
            app.service(serve_path);
        }
    } else {
        // Handle single files
        app.service(web::resource(&full_route).route(web::to(listing::file_handler)));
    }
}

async fn error_404(req: HttpRequest) -> HttpResponse {
    let err_404 = ContextualError::RouteNotFoundError(req.path().to_string());
    let conf = req.app_data::<MiniserveConfig>().unwrap();
    let uses_random_route = conf.random_route.is_some();
    let favicon_route = conf.favicon_route.clone();
    let css_route = conf.css_route.clone();
    let query_params = listing::extract_query_parameters(&req);

    errors::log_error_chain(err_404.to_string());

    actix_web::HttpResponse::NotFound().body(
        renderer::render_error(
            &err_404.to_string(),
            StatusCode::NOT_FOUND,
            "/",
            query_params.sort,
            query_params.order,
            false,
            !uses_random_route,
            &favicon_route,
            &css_route,
            &conf.default_color_scheme,
            &conf.default_color_scheme_dark,
            conf.hide_version_footer,
        )
        .into_string(),
    )
}

async fn favicon() -> impl Responder {
    let logo = include_str!("../data/logo.svg");
    web::HttpResponse::Ok()
        .set(ContentType(mime::IMAGE_SVG))
        .message_body(logo.into())
}

async fn css() -> impl Responder {
    let css = include_str!(concat!(env!("OUT_DIR"), "/style.css"));
    web::HttpResponse::Ok()
        .set(ContentType(mime::TEXT_CSS))
        .message_body(css.into())
}

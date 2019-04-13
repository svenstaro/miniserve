#![feature(proc_macro_hygiene)]

use actix_web::http::Method;
use actix_web::{fs, middleware, server, App};
use clap::crate_version;
use simplelog::{Config, LevelFilter, TermLogger};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;
use std::time::Duration;
use yansi::{Color, Paint};

mod archive;
mod args;
mod auth;
mod errors;
mod file_upload;
mod listing;
mod renderer;
mod themes;

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
    pub auth: Option<auth::BasicAuthParams>,

    /// If false, miniserve will serve the current working directory
    pub path_explicitly_chosen: bool,

    /// Enable symlink resolution
    pub no_symlinks: bool,

    /// Enable random route generation
    pub random_route: Option<String>,

    /// Default color scheme
    pub default_color_scheme: themes::ColorScheme,

    /// Enable file upload
    pub file_upload: bool,

    /// Enable upload to override existing files
    pub overwrite_files: bool,
}

fn main() {
    if cfg!(windows) && !Paint::enable_windows_ascii() {
        Paint::disable();
    }

    let miniserve_config = args::parse_args();

    let _ = if miniserve_config.verbose {
        TermLogger::init(LevelFilter::Info, Config::default())
    } else {
        TermLogger::init(LevelFilter::Error, Config::default())
    };

    if miniserve_config.no_symlinks
        && miniserve_config
            .path
            .symlink_metadata()
            .expect("Can't get file metadata")
            .file_type()
            .is_symlink()
    {
        log::error!("The no-symlinks option cannot be used with a symlink path");
        return;
    }

    let sys = actix::System::new("miniserve");

    let inside_config = miniserve_config.clone();

    let interfaces = miniserve_config
        .interfaces
        .iter()
        .map(|&interface| {
            if interface == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
                // If the interface is 0.0.0.0, we'll change it to 127.0.0.1 so that clicking the link will
                // also work on Windows. Why can't Windows interpret 0.0.0.0?
                String::from("127.0.0.1")
            } else if interface.is_ipv6() {
                // If the interface is IPv6 then we'll print it with brackets so that it is clickable.
                format!("[{}]", interface)
            } else {
                format!("{}", interface)
            }
        })
        .collect::<Vec<String>>();

    let canon_path = miniserve_config.path.canonicalize().unwrap();
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
        io::stdout().flush().unwrap();
        for c in "3… 2… 1… \n".chars() {
            print!("{}", c);
            io::stdout().flush().unwrap();
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
        let random_route = miniserve_config.clone().random_route;
        if random_route.is_some() {
            addresses.push_str(&format!(
                "{}",
                Color::Green
                    .paint(format!(
                        "/{random_route}",
                        random_route = random_route.unwrap(),
                    ))
                    .bold()
            ));
        }
    }
    println!(
        "Serving path {path} at {addresses}",
        path = Color::Yellow.paint(path_string).bold(),
        addresses = addresses,
    );
    println!("\nQuit by pressing CTRL-C");

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

    // Note that this should never fail, since CLI parsing succeeded
    // This means the format of the IP address is valid, and so is the port
    // Valid IpAddr + valid port == valid SocketAddr
    let socket_addresses = socket_addresses.expect("Failed to parse string as socket address");

    server::new(move || {
        App::with_state(inside_config.clone())
            .middleware(auth::Auth)
            .middleware(middleware::Logger::default())
            .configure(configure_app)
    })
    .bind(socket_addresses.as_slice())
    .expect("Couldn't bind server")
    .shutdown_timeout(0)
    .start();
    let _ = sys.run();
}

/// Configures the Actix application
fn configure_app(app: App<MiniserveConfig>) -> App<MiniserveConfig> {
    let upload_route;
    let s = {
        let path = &app.state().path;
        let no_symlinks = app.state().no_symlinks;
        let random_route = app.state().random_route.clone();
        let default_color_scheme = app.state().default_color_scheme.clone();
        let file_upload = app.state().file_upload;
        upload_route = match app.state().random_route.clone() {
            Some(random_route) => format!("/{}/upload", random_route),
            None => "/upload".to_string(),
        };
        if path.is_file() {
            None
        } else {
            let u_r = upload_route.clone();
            Some(
                fs::StaticFiles::new(path)
                    .expect("Couldn't create path")
                    .show_files_listing()
                    .files_listing_renderer(move |dir, req| {
                        listing::directory_listing(
                            dir,
                            req,
                            no_symlinks,
                            file_upload,
                            random_route.clone(),
                            default_color_scheme.clone(),
                            u_r.clone(),
                        )
                    }),
            )
        }
    };

    let random_route = app.state().random_route.clone().unwrap_or_default();
    let full_route = format!("/{}", random_route);

    if let Some(s) = s {
        if app.state().file_upload {
            // Allow file upload
            app.resource(&upload_route, |r| {
                r.method(Method::POST).f(file_upload::upload_file)
            })
            // Handle directories
            .handler(&full_route, s)
        } else {
            // Handle directories
            app.handler(&full_route, s)
        }
    } else {
        // Handle single files
        app.resource(&full_route, |r| r.f(listing::file_handler))
    }
}

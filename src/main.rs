#![feature(proc_macro_hygiene)]

use actix_web::{fs, middleware, server, App};
use clap::crate_version;
use simplelog::{Config, LevelFilter, TermLogger};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::thread;
use std::time::Duration;
use yansi::{Color, Paint};

mod args;
mod auth;
mod listing;
mod renderer;

#[derive(Clone, Debug)]
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

    /// Sort files/directories  
    pub sort_method: listing::SortingMethods,

    /// Enable inverse sorting
    pub reverse_sort: bool,
}

fn main() {
    if cfg!(windows) && !Paint::enable_windows_ascii() {
        Paint::disable();
    }

    let miniserve_config = args::parse_args();
    if miniserve_config.no_symlinks
        && miniserve_config
            .path
            .symlink_metadata()
            .expect("Can't get file metadata")
            .file_type()
            .is_symlink()
    {
        println!(
            "{error} The no-symlinks option cannot be used with a symlink path",
            error = Paint::red("error:").bold(),
        );
        return;
    }

    if miniserve_config.verbose {
        let _ = TermLogger::init(LevelFilter::Info, Config::default());
    }
    let sys = actix::System::new("miniserve");

    let inside_config = miniserve_config.clone();
    server::new(move || {
        App::with_state(inside_config.clone())
            .middleware(auth::Auth)
            .middleware(middleware::Logger::default())
            .configure(configure_app)
    })
    .bind(
        miniserve_config
            .interfaces
            .iter()
            .map(|interface| {
                format!(
                    "{interface}:{port}",
                    interface = &interface,
                    port = miniserve_config.port,
                )
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap()
            })
            .collect::<Vec<SocketAddr>>()
            .as_slice(),
    )
    .expect("Couldn't bind server")
    .shutdown_timeout(0)
    .start();

    let interfaces = miniserve_config.interfaces.iter().map(|&interface| {
        if interface == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
            // If the interface is 0.0.0.0, we'll change it to localhost so that clicking the link will
            // also work on Windows. Why can't Windows interpret 0.0.0.0?
            String::from("localhost")
        } else if interface.is_ipv6() {
            // If the interface is IPv6 then we'll print it with brackets so that it is clickable.
            format!("[{}]", interface)
        } else {
            format!("{}", interface)
        }
    });

    let canon_path = miniserve_config.path.canonicalize().unwrap();
    let path_string = canon_path.to_string_lossy();

    println!(
        "{name} v{version}",
        name = Paint::new("miniserve").bold(),
        version = crate_version!()
    );
    if !miniserve_config.path_explicitly_chosen {
        println!("{info} miniserve has been invoked without an explicit path so it will serve the current directory.", info=Color::Blue.paint("Info:").bold());
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
    for interface in interfaces {
        if !addresses.is_empty() {
            addresses.push_str(", ");
        }
        addresses.push_str(&format!(
            "{}",
            Color::Green
                .paint(format!(
                    "http://{interface}:{port}",
                    interface = interface,
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
    println!("Quit by pressing CTRL-C");

    let _ = sys.run();
}

/// Configures the Actix application
fn configure_app(app: App<MiniserveConfig>) -> App<MiniserveConfig> {
    let s = {
        let path = &app.state().path;
        let no_symlinks = app.state().no_symlinks;
        let random_route = app.state().random_route.clone();
        let sort_method = app.state().sort_method;
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
                            sort_method,
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
        app.resource(&full_route, |r| r.f(listing::file_handler))
    }
}

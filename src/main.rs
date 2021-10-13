use std::io;
use std::io::Write;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::thread;
use std::time::Duration;

use actix_web::web;
use actix_web::{http::header::ContentType, Responder};
use actix_web::{middleware, App, HttpRequest, HttpResponse};
use actix_web_httpauth::middleware::HttpAuthentication;
use anyhow::{bail, Result};
use clap::{crate_version, Clap, IntoApp};
use clap_generate::generators::{Bash, Elvish, Fish, PowerShell, Zsh};
use clap_generate::{generate, Shell};
use log::{error, warn};
use qrcodegen::{QrCode, QrCodeEcc};
use yansi::{Color, Paint};

mod archive;
mod args;
mod auth;
mod config;
mod errors;
mod file_upload;
mod listing;
mod pipe;
mod renderer;

use crate::config::MiniserveConfig;
use crate::errors::ContextualError;

fn main() -> Result<()> {
    let args = args::CliArgs::parse();

    if let Some(shell) = args.print_completions {
        let mut clap_app = args::CliArgs::into_app();
        match shell {
            Shell::Bash => generate::<Bash, _>(&mut clap_app, "miniserve", &mut std::io::stdout()),
            Shell::Elvish => {
                generate::<Elvish, _>(&mut clap_app, "miniserve", &mut std::io::stdout())
            }
            Shell::Fish => generate::<Fish, _>(&mut clap_app, "miniserve", &mut std::io::stdout()),
            Shell::PowerShell => {
                generate::<PowerShell, _>(&mut clap_app, "miniserve", &mut std::io::stdout())
            }
            Shell::Zsh => generate::<Zsh, _>(&mut clap_app, "miniserve", &mut std::io::stdout()),
            _ => bail!("Invalid shell provided!"),
        }
        return Ok(());
    }

    let miniserve_config = MiniserveConfig::try_from_args(args)?;

    run(miniserve_config).map_err(|e| {
        errors::log_error_chain(e.to_string());
        e
    })?;

    Ok(())
}

#[actix_web::main(miniserve)]
async fn run(miniserve_config: MiniserveConfig) -> Result<(), ContextualError> {
    if cfg!(windows) && !Paint::enable_windows_ascii() {
        Paint::disable();
    }

    let log_level = if miniserve_config.verbose {
        simplelog::LevelFilter::Info
    } else {
        simplelog::LevelFilter::Warn
    };

    if simplelog::TermLogger::init(
        log_level,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
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
            return Err(ContextualError::NoSymlinksOptionWithSymlinkServePath(
                miniserve_config.path.to_string_lossy().to_string(),
            ));
        }
    }

    let inside_config = miniserve_config.clone();

    let canon_path = miniserve_config.path.canonicalize().map_err(|e| {
        ContextualError::IoError("Failed to resolve path to be served".to_string(), e)
    })?;

    if let Some(index_path) = &miniserve_config.index {
        let has_index: std::path::PathBuf = [&canon_path, index_path].iter().collect();
        if !has_index.exists() {
            error!(
                "The file '{}' provided for option --index could not be found.",
                index_path.to_string_lossy()
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
        // If the path to serve has NOT been explicitly chosen and if this is NOT an interactive
        // terminal, we should refuse to start for security reasons. This would be the case when
        // running miniserve as a service but forgetting to set the path. This could be pretty
        // dangerous if given with an undesired context path (for instance /root or /).
        if !atty::is(atty::Stream::Stdout) {
            return Err(ContextualError::NoExplicitPathAndNoTerminal);
        }

        warn!("miniserve has been invoked without an explicit path so it will serve the current directory after a short delay.");
        warn!(
            "Invoke with -h|--help to see options or invoke as `miniserve .` to hide this advice."
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

    let display_urls = {
        let (mut ifaces, wildcard): (Vec<_>, Vec<_>) = miniserve_config
            .interfaces
            .clone()
            .into_iter()
            .partition(|addr| !addr.is_unspecified());

        // Replace wildcard addresses with local interface addresses
        if !wildcard.is_empty() {
            let all_ipv4 = wildcard.iter().any(|addr| addr.is_ipv4());
            let all_ipv6 = wildcard.iter().any(|addr| addr.is_ipv6());
            ifaces = get_if_addrs::get_if_addrs()
                .unwrap_or_else(|e| {
                    error!("Failed to get local interface addresses: {}", e);
                    Default::default()
                })
                .into_iter()
                .map(|iface| iface.ip())
                .filter(|ip| (all_ipv4 && ip.is_ipv4()) || (all_ipv6 && ip.is_ipv6()))
                .collect();
            ifaces.sort();
        }

        ifaces
            .into_iter()
            .map(|addr| match addr {
                IpAddr::V4(_) => format!("{}:{}", addr, miniserve_config.port),
                IpAddr::V6(_) => format!("[{}]:{}", addr, miniserve_config.port),
            })
            .map(|addr| match miniserve_config.tls_rustls_config {
                Some(_) => format!("https://{}", addr),
                None => format!("http://{}", addr),
            })
            .map(|url| match miniserve_config.random_route {
                Some(ref random_route) => format!("{}/{}", url, random_route),
                None => url,
            })
            .collect::<Vec<_>>()
    };

    let socket_addresses = miniserve_config
        .interfaces
        .iter()
        .map(|&interface| SocketAddr::new(interface, miniserve_config.port))
        .collect::<Vec<_>>();

    let display_sockets = socket_addresses
        .iter()
        .map(|sock| Color::Green.paint(sock.to_string()).bold().to_string())
        .collect::<Vec<_>>();

    let srv = actix_web::HttpServer::new(move || {
        App::new()
            .wrap(configure_header(&inside_config.clone()))
            .app_data(inside_config.clone())
            .wrap_fn(errors::error_page_middleware)
            .wrap(middleware::Logger::default())
            .route(
                &format!("/{}", inside_config.favicon_route),
                web::get().to(favicon),
            )
            .route(&format!("/{}", inside_config.css_route), web::get().to(css))
            .service(
                web::scope(inside_config.random_route.as_deref().unwrap_or(""))
                    .wrap(middleware::Condition::new(
                        !inside_config.auth.is_empty(),
                        HttpAuthentication::basic(auth::handle_auth),
                    ))
                    .configure(|c| configure_app(c, &inside_config)),
            )
            .default_service(web::get().to(error_404))
    });

    let srv = socket_addresses.iter().try_fold(srv, |srv, addr| {
        let listener = create_tcp_listener(*addr).map_err(|e| {
            ContextualError::IoError(format!("Failed to bind server to {}", addr), e)
        })?;

        #[cfg(feature = "tls")]
        let srv = match &miniserve_config.tls_rustls_config {
            Some(tls_config) => srv.listen_rustls(listener, tls_config.clone()),
            None => srv.listen(listener),
        };

        #[cfg(not(feature = "tls"))]
        let srv = srv.listen(listener);

        srv.map_err(|e| ContextualError::IoError(format!("Failed to bind server to {}", addr), e))
    })?;

    let srv = srv.shutdown_timeout(0).run();

    println!("Bound to {}", display_sockets.join(", "));

    println!("Serving path {}", Color::Yellow.paint(path_string).bold());

    println!(
        "Availabe at (non-exhaustive list):\n    {}\n",
        display_urls
            .iter()
            .map(|url| Color::Green.paint(url).bold().to_string())
            .collect::<Vec<_>>()
            .join("\n    "),
    );

    // print QR code to terminal
    if miniserve_config.show_qrcode && atty::is(atty::Stream::Stdout) {
        for url in display_urls
            .iter()
            .filter(|url| !url.contains("//127.0.0.1:") && !url.contains("//[::1]:"))
        {
            match QrCode::encode_text(url, QrCodeEcc::Low) {
                Ok(qr) => {
                    println!("QR code for {}:", Color::Green.paint(url).bold());
                    print_qr(&qr);
                }
                Err(e) => {
                    error!("Failed to render QR to terminal: {}", e);
                }
            };
        }
    }

    if atty::is(atty::Stream::Stdout) {
        println!("Quit by pressing CTRL-C");
    }

    srv.await
        .map_err(|e| ContextualError::IoError("".to_owned(), e))
}

/// Allows us to set low-level socket options
///
/// This mainly used to set `set_only_v6` socket option
/// to get a consistent behavior across platforms.
/// see: https://github.com/svenstaro/miniserve/pull/500
fn create_tcp_listener(addr: SocketAddr) -> io::Result<TcpListener> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))?;
    if addr.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024 /* Default backlog */)?;
    Ok(TcpListener::from(socket))
}

fn configure_header(conf: &MiniserveConfig) -> middleware::DefaultHeaders {
    conf.header.iter().flatten().fold(
        middleware::DefaultHeaders::new(),
        |headers, (header_name, header_value)| headers.header(header_name, header_value),
    )
}

/// Configures the Actix application
fn configure_app(app: &mut web::ServiceConfig, conf: &MiniserveConfig) {
    let files_service = || {
        let files = actix_files::Files::new("", &conf.path);
        let files = match &conf.index {
            Some(index_file) => files.index_file(index_file.to_string_lossy()),
            None => files,
        };
        let files = match conf.show_hidden {
            true => files.use_hidden_files(),
            false => files,
        };
        files
            .show_files_listing()
            .files_listing_renderer(listing::directory_listing)
            .prefer_utf8(true)
            .redirect_to_slash_directory()
            .default_handler(web::to(error_404))
    };

    if !conf.path.is_file() {
        if conf.file_upload {
            // Allow file upload
            app.service(web::resource("/upload").route(web::post().to(file_upload::upload_file)));
        }
        // Handle directories
        app.service(files_service());
    } else {
        // Handle single files
        app.service(web::resource(["", "/"]).route(web::to(listing::file_handler)));
    }
}

async fn error_404(req: HttpRequest) -> Result<HttpResponse, ContextualError> {
    Err(ContextualError::RouteNotFoundError(req.path().to_string()))
}

async fn favicon() -> impl Responder {
    let logo = include_str!("../data/logo.svg");
    HttpResponse::Ok()
        .insert_header(ContentType(mime::IMAGE_SVG))
        .message_body(logo.into())
}

async fn css() -> impl Responder {
    let css = include_str!(concat!(env!("OUT_DIR"), "/style.css"));
    HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_CSS))
        .message_body(css.into())
}

// Prints to the console two inverted QrCodes side by side.
fn print_qr(qr: &QrCode) {
    let border = 4;
    let size = qr.size() + 2 * border;

    for y in (0..size).step_by(2) {
        for x in 0..2 * size {
            let inverted = x >= size;
            let (x, y) = (x % size - border, y - border);

            //each char represents two vertical modules
            let (mod1, mod2) = match inverted {
                false => (qr.get_module(x, y), qr.get_module(x, y + 1)),
                true => (!qr.get_module(x, y), !qr.get_module(x, y + 1)),
            };
            let c = match (mod1, mod2) {
                (false, false) => ' ',
                (true, false) => '▀',
                (false, true) => '▄',
                (true, true) => '█',
            };
            print!("{0}", c);
        }
        println!();
    }
    println!();
}

use std::io::{self, IsTerminal, Write};
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::thread;
use std::time::Duration;

use actix_files::NamedFile;
use actix_web::middleware::from_fn;
use actix_web::{
    App,
    dev::{ServiceRequest, ServiceResponse, fn_service},
    guard,
    http::Method,
    middleware, web,
};
use actix_web_httpauth::middleware::HttpAuthentication;
use anyhow::Result;
use clap::{CommandFactory, Parser, crate_version};
use colored::*;
use dav_server::{DavHandler, DavMethodSet};
use fast_qr::QRBuilder;
use log::{error, warn};

mod archive;
mod args;
mod auth;
mod config;
mod consts;
mod errors;
mod file_op;
mod file_utils;
mod handlers;
mod listing;
mod pipe;
mod renderer;
mod webdav_fs;

use crate::config::MiniserveConfig;
use crate::errors::StartupError;
use crate::handlers::{
    DirSizeTasks, SseManager, api_command, api_sse, css, dav_handler, error_404, favicon,
    healthcheck,
};
use crate::webdav_fs::RestrictedFs;

static STYLESHEET: &str = grass::include!("data/style.scss");

fn main() -> Result<()> {
    let args = args::CliArgs::parse();

    if let Some(shell) = args.print_completions {
        let mut clap_app = args::CliArgs::command();
        let app_name = clap_app.get_name().to_string();
        clap_complete::generate(shell, &mut clap_app, app_name, &mut io::stdout());
        return Ok(());
    }

    if args.print_manpage {
        let clap_app = args::CliArgs::command();
        let man = clap_mangen::Man::new(clap_app);
        man.render(&mut io::stdout())?;
        return Ok(());
    }

    let miniserve_config = MiniserveConfig::try_from_args(args)?;

    run(miniserve_config).inspect_err(|e| {
        errors::log_error_chain(e.to_string());
    })?;

    Ok(())
}

#[actix_web::main(miniserve)]
async fn run(miniserve_config: MiniserveConfig) -> Result<(), StartupError> {
    let log_level = if miniserve_config.verbose {
        simplelog::LevelFilter::Info
    } else {
        simplelog::LevelFilter::Warn
    };

    simplelog::TermLogger::init(
        log_level,
        simplelog::ConfigBuilder::new()
            .set_time_format_rfc2822()
            .build(),
        simplelog::TerminalMode::Mixed,
        if io::stdout().is_terminal() {
            simplelog::ColorChoice::Auto
        } else {
            simplelog::ColorChoice::Never
        },
    )
    .or_else(|_| simplelog::SimpleLogger::init(log_level, simplelog::Config::default()))
    .expect("Couldn't initialize logger");

    if miniserve_config.no_symlinks && miniserve_config.path.is_symlink() {
        return Err(StartupError::NoSymlinksOptionWithSymlinkServePath(
            miniserve_config.path.to_string_lossy().to_string(),
        ));
    }

    if miniserve_config.webdav_enabled && miniserve_config.path.is_file() {
        return Err(StartupError::WebdavWithFileServePath(
            miniserve_config.path.to_string_lossy().to_string(),
        ));
    }

    let inside_config = miniserve_config.clone();

    let canon_path = miniserve_config
        .path
        .canonicalize()
        .map_err(|e| StartupError::IoError("Failed to resolve path to be served".to_string(), e))?;

    // warn if --index is specified but not found
    if let Some(ref index) = miniserve_config.index {
        if !canon_path.join(index).exists() {
            warn!(
                "The file '{}' provided for option --index could not be found.",
                index.to_string_lossy(),
            );
        }
    }

    let path_string = canon_path.to_string_lossy();

    println!(
        "{name} v{version}",
        name = "miniserve".bold(),
        version = crate_version!()
    );
    if !miniserve_config.path_explicitly_chosen {
        // If the path to serve has NOT been explicitly chosen and if this is NOT an interactive
        // terminal, we should refuse to start for security reasons. This would be the case when
        // running miniserve as a service but forgetting to set the path. This could be pretty
        // dangerous if given with an undesired context path (for instance /root or /).
        if !io::stdout().is_terminal() {
            return Err(StartupError::NoExplicitPathAndNoTerminal);
        }

        warn!(
            "miniserve has been invoked without an explicit path so it will serve the current directory after a short delay."
        );
        warn!(
            "Invoke with -h|--help to see options or invoke as `miniserve .` to hide this advice."
        );
        print!("Starting server in ");
        io::stdout()
            .flush()
            .map_err(|e| StartupError::IoError("Failed to write data".to_string(), e))?;
        for c in "3… 2… 1… \n".chars() {
            print!("{c}");
            io::stdout()
                .flush()
                .map_err(|e| StartupError::IoError("Failed to write data".to_string(), e))?;
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
            ifaces = if_addrs::get_if_addrs()
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
                Some(_) => format!("https://{addr}"),
                None => format!("http://{addr}"),
            })
            .map(|url| format!("{}{}", url, miniserve_config.route_prefix))
            .collect::<Vec<_>>()
    };

    let socket_addresses = miniserve_config
        .interfaces
        .iter()
        .map(|&interface| SocketAddr::new(interface, miniserve_config.port))
        .collect::<Vec<_>>();

    let display_sockets = socket_addresses
        .iter()
        .map(|sock| sock.to_string().green().bold().to_string())
        .collect::<Vec<_>>();

    let stylesheet = web::Data::new(
        [
            STYLESHEET,
            inside_config.default_color_scheme.css(),
            inside_config.default_color_scheme_dark.css_dark().as_str(),
        ]
        .join("\n"),
    );

    let sse_manager = web::Data::new(SseManager::new());
    let dir_size_tasks = web::Data::new(DirSizeTasks::new(
        miniserve_config.show_exact_bytes,
        sse_manager.clone(),
    ));

    let srv = actix_web::HttpServer::new(move || {
        App::new()
            .wrap(configure_header(&inside_config.clone()))
            .app_data(web::Data::new(inside_config.clone()))
            .app_data(dir_size_tasks.clone())
            .app_data(sse_manager.clone())
            .app_data(stylesheet.clone())
            .wrap(from_fn(errors::error_page_middleware))
            .wrap(middleware::Logger::default())
            .wrap(middleware::Condition::new(
                miniserve_config.compress_response,
                middleware::Compress::default(),
            ))
            .route(&inside_config.healthcheck_route, web::get().to(healthcheck))
            .route(&inside_config.api_route, web::post().to(api_command))
            .route(&inside_config.api_route, web::get().to(api_sse))
            .route(&inside_config.favicon_route, web::get().to(favicon))
            .route(&inside_config.css_route, web::get().to(css))
            .service(
                web::scope(&inside_config.route_prefix)
                    .wrap(middleware::Condition::new(
                        !inside_config.auth.is_empty(),
                        actix_web::middleware::Compat::new(HttpAuthentication::basic(
                            auth::handle_auth,
                        )),
                    ))
                    .configure(|c| configure_app(c, &inside_config)),
            )
            .default_service(web::get().to(error_404))
    });

    let srv = socket_addresses.iter().try_fold(srv, |srv, addr| {
        let listener = create_tcp_listener(*addr)
            .map_err(|e| StartupError::IoError(format!("Failed to bind server to {addr}"), e))?;

        #[cfg(feature = "tls")]
        let srv = match &miniserve_config.tls_rustls_config {
            Some(tls_config) => srv.listen_rustls_0_23(listener, tls_config.clone()),
            None => srv.listen(listener),
        };

        #[cfg(not(feature = "tls"))]
        let srv = srv.listen(listener);

        srv.map_err(|e| StartupError::IoError(format!("Failed to bind server to {addr}"), e))
    })?;

    let srv = srv.shutdown_timeout(0).run();

    println!("Bound to {}", display_sockets.join(", "));

    println!("Serving path {}", path_string.yellow().bold());

    println!(
        "Available at (non-exhaustive list):\n    {}\n",
        display_urls
            .iter()
            .map(|url| url.green().bold().to_string())
            .collect::<Vec<_>>()
            .join("\n    "),
    );

    // print QR code to terminal
    if miniserve_config.show_qrcode && io::stdout().is_terminal() {
        for url in display_urls
            .iter()
            .filter(|url| !url.contains("//127.0.0.1:") && !url.contains("//[::1]:"))
        {
            match QRBuilder::new(url.clone()).ecl(consts::QR_EC_LEVEL).build() {
                Ok(qr) => {
                    println!("QR code for {}:", url.green().bold());
                    qr.print();
                }
                Err(e) => {
                    error!("Failed to render QR to terminal: {:?}", e);
                }
            };
        }
    }

    if io::stdout().is_terminal() {
        println!("Quit by pressing CTRL-C");
    }

    srv.await
        .map_err(|e| StartupError::IoError("".to_owned(), e))
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
        |headers, (header_name, header_value)| headers.add((header_name, header_value)),
    )
}

/// Configures the Actix application
///
/// This is where we configure the app to serve an index file, the file listing, or a single file.
fn configure_app(app: &mut web::ServiceConfig, conf: &MiniserveConfig) {
    let dir_service = || {
        // use routing guard so propfind and options requests fall through to the webdav handler
        let mut files = actix_files::Files::new("", &conf.path)
            .guard(guard::Any(guard::Get()).or(guard::Head()));

        // Use specific index file if one was provided.
        if let Some(ref index_file) = conf.index {
            files = files.index_file(index_file.to_string_lossy());
            // Handle SPA option.
            //
            // Note: --spa requires --index in clap.
            if conf.spa {
                files = files.default_handler(
                    NamedFile::open(conf.path.join(index_file))
                        .expect("Can't open SPA index file."),
                );
            }
        }

        // Handle --pretty-urls options.
        //
        // We rewrite the request to append ".html" to the path and serve the file. If the
        // path ends with a `/`, we remove it before appending ".html".
        //
        // This is done to allow for pretty URLs, e.g. "/about" instead of "/about.html".
        if conf.pretty_urls {
            files = files.default_handler(fn_service(|req: ServiceRequest| async {
                let (req, _) = req.into_parts();
                let conf = req
                    .app_data::<web::Data<MiniserveConfig>>()
                    .expect("Could not get miniserve config");
                let mut path_base = req.path()[1..].to_string();
                if path_base.ends_with('/') {
                    path_base.pop();
                }
                if !path_base.ends_with("html") {
                    path_base = format!("{}.html", path_base);
                }
                let file = NamedFile::open_async(conf.path.join(path_base)).await?;
                let res = file.into_response(&req);
                Ok(ServiceResponse::new(req, res))
            }));
        }

        if conf.show_hidden {
            files = files.use_hidden_files();
        }

        let base_path = conf.path.clone();
        let no_symlinks = conf.no_symlinks;
        files
            .show_files_listing()
            .files_listing_renderer(listing::directory_listing)
            .prefer_utf8(true)
            .redirect_to_slash_directory()
            .path_filter(move |path, _| {
                // deny symlinks if conf.no_symlinks
                !(no_symlinks && base_path.join(path).is_symlink())
            })
    };

    if conf.path.is_file() {
        // Handle single files
        app.service(web::resource(["", "/"]).route(web::to(listing::file_handler)));
    } else {
        if conf.file_upload {
            // Allow file upload
            app.service(web::resource("/upload").route(web::post().to(file_op::upload_file)));
        }
        // Handle directories
        app.service(dir_service());
    }

    if conf.webdav_enabled {
        let fs = RestrictedFs::new(&conf.path, conf.show_hidden);

        let dav_server = DavHandler::builder()
            .filesystem(fs)
            .methods(DavMethodSet::WEBDAV_RO)
            .hide_symlinks(conf.no_symlinks)
            .strip_prefix(conf.route_prefix.to_owned())
            .build_handler();

        app.app_data(web::Data::new(dav_server.clone()));

        app.service(
            // actix requires tail segment to be named, even if unused
            web::resource("/{tail}*")
                .guard(
                    guard::Any(guard::Options())
                        .or(guard::Method(Method::from_bytes(b"PROPFIND").unwrap())),
                )
                .to(dav_handler),
        );
    }
}

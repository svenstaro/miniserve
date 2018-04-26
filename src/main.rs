extern crate actix;
extern crate actix_web;
extern crate simplelog;

#[macro_use]
extern crate clap;

use actix_web::{server, App, fs, middleware, HttpRequest, Result};
use simplelog::{TermLogger, LevelFilter, Config};
use std::path::PathBuf;
use std::net::IpAddr;

#[derive(Clone)]
pub struct MiniserveConfig {
    verbose: bool,
    path: std::path::PathBuf,
    port: u16,
    interface: IpAddr,
}

fn is_valid_path(path: String) -> Result<(), String> {
    let path_to_check = PathBuf::from(path);
    if path_to_check.is_file() || path_to_check.is_dir() {
        return Ok(());
    }
    Err(String::from("Path either doesn't exist or is not a regular file or a directory"))
}

fn is_valid_port(port: String) -> Result<(), String> {
    port.parse::<u16>().and(Ok(())).or_else(|e| Err(e.to_string()))
}

fn is_valid_interface(interface: String) -> Result<(), String> {
    interface.parse::<IpAddr>().and(Ok(())).or_else(|e| Err(e.to_string()))
}

pub fn parse_args() -> MiniserveConfig {
    use clap::{App, Arg};

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Be verbose"),
        )
        .arg(
            Arg::with_name("PATH")
				.required(true)
                .validator(is_valid_path)
                .help("Which path to serve"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port to use")
                .validator(is_valid_port)
                .required(false)
                .default_value("8080")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("interface")
                .short("i")
                .long("if")
                .help("Interface to listen on")
                .validator(is_valid_interface)
                .required(false)
                .default_value("0.0.0.0")
                .takes_value(true),
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    let path = matches.value_of("PATH").unwrap();
    let port = matches.value_of("port").unwrap().parse().unwrap();
    let interface = matches.value_of("interface").unwrap().parse().unwrap();

    MiniserveConfig {
        verbose,
        path: PathBuf::from(path),
        port,
        interface,
    }
}

fn file_handler(req: HttpRequest<MiniserveConfig>) -> Result<fs::NamedFile> {
    let path = &req.state().path;
    Ok(fs::NamedFile::open(path)?)
}

fn configure_app(app: App<MiniserveConfig>) -> App<MiniserveConfig> {
    let s = {
        let path = &app.state().path;
        if path.is_file() {
            None
        } else {
            Some(fs::StaticFiles::new(path).show_files_listing())
        }
    };

    if let Some(s) = s {
        app.handler("/", s)
    } else {
        app.resource("/", |r| r.f(file_handler))
    }
}

fn main() {
    let miniserve_config = parse_args();

    if miniserve_config.verbose {
        let _ = TermLogger::init(LevelFilter::Info, Config::default());
    }
    let sys = actix::System::new("miniserve");

    let inside_config = miniserve_config.clone();
	server::new(
		move || App::with_state(inside_config.clone())
            .middleware(middleware::Logger::default())
            .configure(configure_app))
		.bind(format!("{}:{}", &miniserve_config.interface, miniserve_config.port)).expect("Couldn't bind server")
		.shutdown_timeout(1)
		.start();

	let _ = sys.run();
}

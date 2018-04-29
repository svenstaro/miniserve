extern crate actix;
extern crate actix_web;
extern crate simplelog;
extern crate base64;

#[macro_use]
extern crate clap;

use actix_web::http::{StatusCode, header};
use actix_web::{server, App, fs, middleware, HttpRequest, HttpResponse, HttpMessage, Result};
use actix_web::middleware::{Middleware, Response};
use simplelog::{TermLogger, LevelFilter, Config};
use std::path::PathBuf;
use std::net::{IpAddr, Ipv4Addr};
use std::error::Error;

/// Decode a HTTP basic auth string into a tuple of username and password.
fn parse_basic_auth(auth: String) -> Result<(String, String), String> {
    let decoded = base64::decode(&auth).map_err(|e| e.description().to_owned())?;
    let decoded_str = String::from_utf8_lossy(&decoded);
    let strings: Vec<&str> = decoded_str.splitn(2, ':').collect();
    if strings.len() != 2 {
        return Err("Invalid username/password format".to_owned());
    }
    let (user, password) = (strings[0], strings[1]);
    Ok((user.to_owned(), password.to_owned()))
}

#[derive(Clone)]
pub struct MiniserveConfig {
    verbose: bool,
    path: std::path::PathBuf,
    port: u16,
    interface: IpAddr,
    auth: Option<String>,
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

fn is_valid_auth(auth: String) -> Result<(), String> {
    auth.find(':').ok_or("Correct format is user:password".to_owned()).map(|_| ())
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
        .arg(
            Arg::with_name("auth")
                .short("a")
                .long("auth")
                .validator(is_valid_auth)
                .help("Set authentication (user:password)")
                .takes_value(true),
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    let path = matches.value_of("PATH").unwrap();
    let port = matches.value_of("port").unwrap().parse().unwrap();
    let interface = matches.value_of("interface").unwrap().parse().unwrap();
    let auth = matches.value_of("auth").map(|a| a.to_owned());

    MiniserveConfig {
        verbose,
        path: PathBuf::from(path),
        port,
        interface,
        auth,
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

struct Auth;

impl Middleware<MiniserveConfig> for Auth {
    fn response(&self, req: &mut HttpRequest<MiniserveConfig>, mut resp: HttpResponse) -> Result<Response> {
        let required_auth = &req.state().auth;
        if required_auth.is_some() {
            // parse_basic_auth(pass)
            println!("{:?}", required_auth);
            println!("{:?}", req.headers().get(header::AUTHORIZATION));
        }
        resp.headers_mut().insert(header::WWW_AUTHENTICATE, header::HeaderValue::from_static("Basic realm=\"lol\""));
        *resp.status_mut() = StatusCode::UNAUTHORIZED;
        Ok(Response::Done(resp))
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
            .middleware(Auth)
            .middleware(middleware::Logger::default())
            .configure(configure_app))
		.bind(format!("{}:{}", &miniserve_config.interface, miniserve_config.port)).expect("Couldn't bind server")
		.shutdown_timeout(0)
		.start();

    // If the interface is 0.0.0.0, we'll change it to localhost so that clicking the link will
    // also work on Windows. Why can't Windows interpret 0.0.0.0?
    let interface =  if miniserve_config.interface == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
        String::from("localhost")
    } else {
        format!("{}", miniserve_config.interface)
    };

    let canon_path = miniserve_config.path.canonicalize().unwrap();
    println!("miniserve is serving your files at http://{interface}:{port}", interface=interface, port=miniserve_config.port);
    println!("Currently serving path {path}", path=canon_path.to_string_lossy());
    println!("Quit by pressing CTRL-C");

	let _ = sys.run();
}

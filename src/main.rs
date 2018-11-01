extern crate actix;
extern crate actix_web;
extern crate base64;
extern crate simplelog;
extern crate yansi;
#[macro_use]
extern crate clap;

use actix_web::http::header;
use actix_web::middleware::{Middleware, Response};
use actix_web::{fs, middleware, server, App, HttpMessage, HttpRequest, HttpResponse, Result};
use simplelog::{Config, LevelFilter, TermLogger};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use yansi::{Color, Paint};

enum BasicAuthError {
    Base64DecodeError,
    InvalidUsernameFormat,
}

#[derive(Clone, Debug)]
struct BasicAuthParams {
    username: String,
    password: String,
}

#[derive(Clone, Debug)]
pub struct MiniserveConfig {
    verbose: bool,
    path: std::path::PathBuf,
    port: u16,
    interfaces: Vec<IpAddr>,
    auth: Option<BasicAuthParams>,
    path_explicitly_chosen: bool,
}

/// Decode a HTTP basic auth string into a tuple of username and password.
fn parse_basic_auth(
    authorization_header: &header::HeaderValue,
) -> Result<BasicAuthParams, BasicAuthError> {
    let basic_removed = authorization_header.to_str().unwrap().replace("Basic ", "");
    let decoded = base64::decode(&basic_removed).map_err(|_| BasicAuthError::Base64DecodeError)?;
    let decoded_str = String::from_utf8_lossy(&decoded);
    let strings: Vec<&str> = decoded_str.splitn(2, ':').collect();
    if strings.len() != 2 {
        return Err(BasicAuthError::InvalidUsernameFormat);
    }
    Ok(BasicAuthParams {
        username: strings[0].to_owned(),
        password: strings[1].to_owned(),
    })
}

fn is_valid_path(path: String) -> Result<(), String> {
    let path_to_check = PathBuf::from(path);
    if path_to_check.is_file() || path_to_check.is_dir() {
        return Ok(());
    }
    Err(String::from(
        "Path either doesn't exist or is not a regular file or a directory",
    ))
}

fn is_valid_port(port: String) -> Result<(), String> {
    port.parse::<u16>()
        .and(Ok(()))
        .or_else(|e| Err(e.to_string()))
}

fn is_valid_interface(interface: String) -> Result<(), String> {
    interface
        .parse::<IpAddr>()
        .and(Ok(()))
        .or_else(|e| Err(e.to_string()))
}

fn is_valid_auth(auth: String) -> Result<(), String> {
    auth.find(':')
        .ok_or_else(|| "Correct format is username:password".to_owned())
        .map(|_| ())
}

pub fn parse_args() -> MiniserveConfig {
    use clap::{App, Arg, AppSettings};

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .global_setting(AppSettings::ColoredHelp)
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Be verbose, includes emitting access logs"),
        ).arg(
            Arg::with_name("PATH")
                .required(false)
                .validator(is_valid_path)
                .help("Which path to serve"),
        ).arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port to use")
                .validator(is_valid_port)
                .required(false)
                .default_value("8080")
                .takes_value(true),
        ).arg(
            Arg::with_name("interfaces")
                .short("i")
                .long("if")
                .help("Interface to listen on")
                .validator(is_valid_interface)
                .required(false)
                .takes_value(true)
                .multiple(true),
        ).arg(
            Arg::with_name("auth")
                .short("a")
                .long("auth")
                .validator(is_valid_auth)
                .help("Set authentication (username:password)")
                .takes_value(true),
        ).get_matches();

    let verbose = matches.is_present("verbose");
    let path = matches.value_of("PATH");
    let port = matches.value_of("port").unwrap().parse().unwrap();
    let interfaces = if let Some(interfaces) = matches.values_of("interfaces") {
        interfaces.map(|x| x.parse().unwrap()).collect()
    } else {
        vec![
            IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        ]
    };
    let auth = if let Some(auth_split) = matches.value_of("auth").map(|x| x.splitn(2, ':')) {
        let auth_vec = auth_split.collect::<Vec<&str>>();
        if auth_vec.len() == 2 {
            Some(BasicAuthParams {
                username: auth_vec[0].to_owned(),
                password: auth_vec[1].to_owned(),
            })
        } else {
            None
        }
    } else {
        None
    };

    MiniserveConfig {
        verbose,
        path: PathBuf::from(path.unwrap_or(".")),
        port,
        interfaces,
        auth,
        path_explicitly_chosen: path.is_some(),
    }
}

fn file_handler(req: &HttpRequest<MiniserveConfig>) -> Result<fs::NamedFile> {
    let path = &req.state().path;
    Ok(fs::NamedFile::open(path)?)
}

fn configure_app(app: App<MiniserveConfig>) -> App<MiniserveConfig> {
    let s = {
        let path = &app.state().path;
        if path.is_file() {
            None
        } else {
            Some(
                fs::StaticFiles::new(path)
                    .expect("Couldn't create path")
                    .show_files_listing(),
            )
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
    fn response(&self, req: &HttpRequest<MiniserveConfig>, resp: HttpResponse) -> Result<Response> {
        if let Some(ref required_auth) = req.state().auth {
            if let Some(auth_headers) = req.headers().get(header::AUTHORIZATION) {
                let auth_req = match parse_basic_auth(auth_headers) {
                    Ok(auth_req) => auth_req,
                    Err(BasicAuthError::Base64DecodeError) => {
                        return Ok(Response::Done(HttpResponse::BadRequest().body(format!(
                            "Error decoding basic auth base64: '{}'",
                            auth_headers.to_str().unwrap()
                        ))))
                    }
                    Err(BasicAuthError::InvalidUsernameFormat) => {
                        return Ok(Response::Done(
                            HttpResponse::BadRequest().body("Invalid basic auth format"),
                        ))
                    }
                };
                if auth_req.username != required_auth.username
                    || auth_req.password != required_auth.password
                {
                    let new_resp = HttpResponse::Forbidden().finish();
                    return Ok(Response::Done(new_resp));
                }
            } else {
                let new_resp = HttpResponse::Unauthorized()
                    .header(
                        header::WWW_AUTHENTICATE,
                        header::HeaderValue::from_static("Basic realm=\"miniserve\""),
                    ).finish();
                return Ok(Response::Done(new_resp));
            }
        }
        Ok(Response::Done(resp))
    }
}

fn main() {
    if cfg!(windows) && !Paint::enable_windows_ascii() {
        Paint::disable();
    }

    let miniserve_config = parse_args();

    if miniserve_config.verbose {
        let _ = TermLogger::init(LevelFilter::Info, Config::default());
    }
    let sys = actix::System::new("miniserve");

    let inside_config = miniserve_config.clone();
    server::new(move || {
        App::with_state(inside_config.clone())
            .middleware(Auth)
            .middleware(middleware::Logger::default())
            .configure(configure_app)
    }).bind(
        miniserve_config
            .interfaces
            .iter()
            .map(|interface| {
                format!(
                    "{interface}:{port}",
                    interface = &interface,
                    port = miniserve_config.port,
                ).to_socket_addrs()
                .unwrap()
                .next()
                .unwrap()
            }).collect::<Vec<SocketAddr>>()
            .as_slice(),
    ).expect("Couldn't bind server")
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
                )).bold()
        ));
    }
    println!(
        "Serving path {path} at {addresses}",
        path = Color::Yellow.paint(path_string).bold(),
        addresses = addresses,
    );
    println!("Quit by pressing CTRL-C");

    let _ = sys.run();
}

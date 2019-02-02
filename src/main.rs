use actix_web::http::header;
use actix_web::middleware::{Middleware, Response};
use actix_web::{fs, middleware, server, App, HttpMessage, HttpRequest, HttpResponse, Result};
use clap::{crate_authors, crate_description, crate_name, crate_version};
use htmlescape::encode_minimal as escape_html_entity;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use simplelog::{Config, LevelFilter, TermLogger};
use std::fmt::Write as FmtWrite;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
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
    no_symlinks: bool,
    random_route: Option<String>,
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
    use clap::{App, AppSettings, Arg};

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
        )
        .arg(
            Arg::with_name("PATH")
                .required(false)
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
            Arg::with_name("interfaces")
                .short("i")
                .long("if")
                .help("Interface to listen on")
                .validator(is_valid_interface)
                .required(false)
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("auth")
                .short("a")
                .long("auth")
                .validator(is_valid_auth)
                .help("Set authentication (username:password)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("no-symlinks")
                .short("P")
                .long("no-symlinks")
                .help("Do not follow symbolic links"),
        )
        .arg(
            Arg::with_name("random-route")
                .short("r")
                .long("random-route")
                .help("Generate a random route"),
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    let no_symlinks = matches.is_present("no-symlinks");
    let path = matches.value_of("PATH");
    let port = matches.value_of("port").unwrap().parse().unwrap();
    let generate_random = matches.is_present("random-route");

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

    let mut random_route = None;

    if generate_random {
        random_route = Some(nanoid::simple());
    }

    MiniserveConfig {
        verbose,
        path: PathBuf::from(path.unwrap_or(".")),
        port,
        interfaces,
        auth,
        path_explicitly_chosen: path.is_some(),
        no_symlinks,
        random_route,
    }
}

fn file_handler(req: &HttpRequest<MiniserveConfig>) -> Result<fs::NamedFile> {
    let path = &req.state().path;
    Ok(fs::NamedFile::open(path)?)
}

fn configure_app(app: App<MiniserveConfig>) -> App<MiniserveConfig> {
    let s = {
        let path = &app.state().path;
        let no_symlinks = app.state().no_symlinks;
        if path.is_file() {
            None
        } else {
            Some(
                fs::StaticFiles::new(path)
                    .expect("Couldn't create path")
                    .show_files_listing()
                    .files_listing_renderer(move |dir, req| {
                        directory_listing(dir, req, no_symlinks)
                    }),
            )
        }
    };

    let route_url = "/".to_string();
    let random_route = app.state().random_route.clone().unwrap_or(String::new());
    let full_route = format!("{}{}", route_url, random_route);

    if let Some(s) = s {
        app.handler(&full_route, s)
    } else {
        app.resource(&full_route, |r| r.f(file_handler))
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
                        ))));
                    }
                    Err(BasicAuthError::InvalidUsernameFormat) => {
                        return Ok(Response::Done(
                            HttpResponse::BadRequest().body("Invalid basic auth format"),
                        ));
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
                    )
                    .finish();
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
            .middleware(Auth)
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

// ↓ Adapted from https://docs.rs/actix-web/0.7.13/src/actix_web/fs.rs.html#564
fn directory_listing<S>(
    dir: &fs::Directory,
    req: &HttpRequest<S>,
    skip_symlinks: bool,
) -> Result<HttpResponse, io::Error> {
    let index_of = format!("Index of {}", req.path());
    let mut body = String::new();
    let base = Path::new(req.path());

    for entry in dir.path.read_dir()? {
        if dir.is_visible(&entry) {
            let entry = entry.unwrap();
            let p = match entry.path().strip_prefix(&dir.path) {
                Ok(p) => base.join(p),
                Err(_) => continue,
            };
            // show file url as relative to static path
            let file_url =
                utf8_percent_encode(&p.to_string_lossy(), DEFAULT_ENCODE_SET).to_string();
            // " -- &quot;  & -- &amp;  ' -- &#x27;  < -- &lt;  > -- &gt;
            let file_name = escape_html_entity(&entry.file_name().to_string_lossy());

            // if file is a directory, add '/' to the end of the name
            if let Ok(metadata) = entry.metadata() {
                if skip_symlinks && metadata.file_type().is_symlink() {
                    continue;
                }
                if metadata.is_dir() {
                    let _ = write!(body, "<li><a href=\"{}\">{}/</a></li>", file_url, file_name);
                } else {
                    let _ = write!(body, "<li><a href=\"{}\">{}</a></li>", file_url, file_name);
                }
            } else {
                continue;
            }
        }
    }

    let html = format!(
        "<html>\
         <head><title>{}</title></head>\
         <body><h1>{}</h1>\
         <ul>\
         {}\
         </ul></body>\n</html>",
        index_of, index_of, body
    );
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

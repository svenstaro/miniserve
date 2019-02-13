use crate::auth;
use crate::listing;
use clap::{crate_authors, crate_description, crate_name, crate_version};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

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

pub fn parse_args() -> crate::MiniserveConfig {
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
            Arg::with_name("random-route")
                .long("random-route")
                .help("Generate a random 6-hexdigit route"),
        )
        .arg(
            Arg::with_name("sort")
                .short("s")
                .long("sort")
                .possible_values(&["natural", "alpha", "dirsfirst"])
                .default_value("natural")
                .help("Sort files"),
        )
        .arg(
            Arg::with_name("reverse")
                .long("reverse")
                .help("Reverse sorting order"),
        )
        .arg(
            Arg::with_name("no-symlinks")
                .short("P")
                .long("no-symlinks")
                .help("Do not follow symbolic links"),
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    let no_symlinks = matches.is_present("no-symlinks");
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
            Some(auth::BasicAuthParams {
                username: auth_vec[0].to_owned(),
                password: auth_vec[1].to_owned(),
            })
        } else {
            None
        }
    } else {
        None
    };

    let random_route = if matches.is_present("random-route") {
        Some(nanoid::custom(6, &ROUTE_ALPHABET))
    } else {
        None
    };

    let sort_method = matches
        .value_of("sort")
        .unwrap()
        .parse::<listing::SortingMethods>()
        .unwrap();

    let reverse_sort = matches.is_present("reverse");

    crate::MiniserveConfig {
        verbose,
        path: PathBuf::from(path.unwrap_or(".")),
        port,
        interfaces,
        auth,
        path_explicitly_chosen: path.is_some(),
        no_symlinks,
        random_route,
        sort_method,
        reverse_sort,
    }
}

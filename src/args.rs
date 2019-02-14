use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use structopt::StructOpt;

use crate::auth;
use crate::listing;

/// Possible characters for random routes
const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[derive(StructOpt, Debug)]
#[structopt(name = "miniserve")]
struct CLIArgs {
    /// Be verbose, includes emitting access logs
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
    /// Which path to serve
    #[structopt(name = "PATH", parse(from_os_str))]
    path: Option<PathBuf>,
    /// Port to use
    #[structopt(short = "p", long = "port", default_value = "8080")]
    port: u16,
    /// Interface to listen on
    #[structopt(short = "i", long = "if", parse(try_from_str = "parse_interface"))]
    interfaces: Vec<IpAddr>,
    /// Set authentication (username:password)
    #[structopt(short = "a", long = "auth", parse(try_from_str = "parse_auth"))]
    auth: Option<(String, String)>,
    /// Generate a random 6-hexdigit route
    #[structopt(long = "random-route")]
    random_route: bool,
    /// Sort files
    #[structopt(
        short = "s",
        long = "sort",
        raw(
            possible_values = "&listing::SortingMethods::variants()",
            case_insensitive = "true"
        )
    )]
    sort_method: Option<listing::SortingMethods>,
    /// Reverse sorting
    #[structopt(long = "reverse")]
    reverse_sort: bool,
    /// Do not follow symbolic links
    #[structopt(short = "P", long = "no-symlinks")]
    no_symlinks: bool,
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

/// Checks wether the auth string is valid, i.e. it follows the syntax username:password
fn parse_auth(src: &str) -> Result<(String, String), String> {
    match src.find(':') {
        Some(_) => {
            let split = src.split(':').collect::<Vec<_>>();
            Ok((split[0].to_owned(), split[1].to_owned()))
        }
        None => Err("Correct format is username:password".to_owned()),
    }
}

/// Parses the command line arguments
pub fn parse_args() -> crate::MiniserveConfig {
    let args = CLIArgs::from_args();

    let interfaces = if !args.interfaces.is_empty() {
        args.interfaces
    } else {
        vec![
            IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        ]
    };

    let auth = match args.auth {
        Some((username, password)) => Some(auth::BasicAuthParams { username, password }),
        None => None,
    };

    let random_route = if args.random_route {
        Some(nanoid::custom(6, &ROUTE_ALPHABET))
    } else {
        None
    };

    let path_explicitly_chosen = args.path.is_some();

    crate::MiniserveConfig {
        verbose: args.verbose,
        path: args.path.unwrap_or_else(|| PathBuf::from(".")),
        port: args.port,
        interfaces,
        auth,
        path_explicitly_chosen,
        no_symlinks: args.no_symlinks,
        random_route,
        sort_method: args.sort_method.unwrap_or(listing::SortingMethods::Natural),
        reverse_sort: args.reverse_sort,
    }
}

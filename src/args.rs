use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use structopt::StructOpt;

use crate::auth;
use crate::themes;

/// Possible characters for random routes
const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[derive(StructOpt)]
#[structopt(
    name = "miniserve",
    raw(global_settings = "&[structopt::clap::AppSettings::ColoredHelp]")
)]
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
    #[structopt(
        short = "i",
        long = "if",
        parse(try_from_str = "parse_interface"),
        raw(number_of_values = "1")
    )]
    interfaces: Vec<IpAddr>,

    /// Set authentication (username:password)
    #[structopt(short = "a", long = "auth", parse(try_from_str = "parse_auth"))]
    auth: Option<auth::RequiredAuth>,

    /// Generate a random 6-hexdigit route
    #[structopt(long = "random-route")]
    random_route: bool,

    /// Do not follow symbolic links
    #[structopt(short = "P", long = "no-symlinks")]
    no_symlinks: bool,

    /// Default color scheme
    #[structopt(
        short = "c",
        long = "color-scheme",
        default_value = "Squirrel",
        raw(
            possible_values = "&themes::ColorScheme::variants()",
            case_insensitive = "true",
        )
    )]
    color_scheme: themes::ColorScheme,

    /// Enable file uploading
    #[structopt(short = "u", long = "upload-files")]
    file_upload: bool,

    /// Enable overriding existing files during file upload
    #[structopt(short = "o", long = "overwrite-files")]
    overwrite_files: bool,
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

/// Checks wether the auth string is valid, i.e. it follows the syntax username:password
fn parse_auth(src: &str) -> Result<auth::RequiredAuth, String> {
    let mut split = src.splitn(3, ':');
    let errmsg = "Invalid credentials string, expected format is username:password".to_owned();

    let username = match split.next() {
        Some(username) => username,
        None => return Err(errmsg),
    };

    let second_part = match split.next() {
        // This allows empty passwords, as the spec does not forbid it
        Some(password) => password,
        None => return Err(errmsg),
    };

    let password = if let Some(hash) = split.next() {
        match second_part {
            "sha256" => auth::RequiredAuthPassword::Sha256(hash.to_owned()),
            "sha512" => auth::RequiredAuthPassword::Sha512(hash.to_owned()),
            _ => return Err("Invalid hash method, only accept either sha256 or sha512".to_owned())
        }
    } else {
        // To make it Windows-compatible, the password needs to be shorter than 255 characters.
        // After 255 characters, Windows will truncate the value.
        // As for the username, the spec does not mention a limit in length
        if second_part.len() > 255 {
            return Err("Password length cannot exceed 255 characters".to_owned());
        }

        auth::RequiredAuthPassword::Plain(second_part.to_owned())
    };

    Ok(auth::RequiredAuth {
        username: username.to_owned(),
        password,
    })
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

    let random_route = if args.random_route {
        Some(nanoid::custom(6, &ROUTE_ALPHABET))
    } else {
        None
    };

    let default_color_scheme = args.color_scheme;

    let path_explicitly_chosen = args.path.is_some();

    crate::MiniserveConfig {
        verbose: args.verbose,
        path: args.path.unwrap_or_else(|| PathBuf::from(".")),
        port: args.port,
        interfaces,
        auth: args.auth,
        path_explicitly_chosen,
        no_symlinks: args.no_symlinks,
        random_route,
        default_color_scheme,
        overwrite_files: args.overwrite_files,
        file_upload: args.file_upload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_required_auth (username: &str, password: &str, encrypt: &str) -> auth::RequiredAuth {
        use auth::*;
        use RequiredAuthPassword::*;

        RequiredAuth {
            username: username.to_owned(),
            password: match encrypt {
                "plain" => Plain(password.to_owned()),
                "sha256" => Sha256(password.to_owned()),
                "sha512" => Sha512(password.to_owned()),
                _ => panic!("Unknown encryption type")
            },
        }
    }

    #[test]
    fn parse_auth_plain() -> Result<(), String> {
        assert_eq!(
            parse_auth("username:password")?,
            create_required_auth("username", "password", "plain")
        );

        Ok(())
    }

    #[test]
    fn parse_auth_sha256() -> Result<(), String> {
        assert_eq!(
            parse_auth("username:sha256:hash")?,
            create_required_auth("username", "hash", "sha256")
        );

        Ok(())
    }

    #[test]
    fn parse_auth_sha512() -> Result<(), String> {
        assert_eq!(
            parse_auth("username:sha512:hash")?,
            create_required_auth("username", "hash", "sha512")
        );

        Ok(())
    }
}
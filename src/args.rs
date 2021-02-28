use bytes::Bytes;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use port_check::free_local_port;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use structopt::StructOpt;

use crate::auth;
use crate::errors::ContextualError;
use crate::renderer;

/// Possible characters for random routes
const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[derive(StructOpt)]
#[structopt(
    name = "miniserve",
    author,
    about,
    global_settings = &[structopt::clap::AppSettings::ColoredHelp],
)]
struct CliArgs {
    /// Be verbose, includes emitting access logs
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,

    /// Which path to serve
    #[structopt(name = "PATH", parse(from_os_str))]
    path: Option<PathBuf>,

    /// The name of a directory index file to serve, like "index.html"
    ///
    /// Normally, when miniserve serves a directory, it creates a listing for that directory.
    /// However, if a directory contains this file, miniserve will serve that file instead.
    #[structopt(long, parse(from_os_str), name = "index_file")]
    index: Option<PathBuf>,

    /// Port to use
    #[structopt(short = "p", long = "port", default_value = "8080")]
    port: u16,

    /// Interface to listen on
    #[structopt(
        short = "i",
        long = "interfaces",
        parse(try_from_str = parse_interface),
        number_of_values = 1,
    )]
    interfaces: Vec<IpAddr>,

    /// Set authentication. Currently supported formats:
    /// username:password, username:sha256:hash, username:sha512:hash
    /// (e.g. joe:123, joe:sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3)
    #[structopt(
        short = "a",
        long = "auth",
        parse(try_from_str = parse_auth),
        number_of_values = 1,
    )]
    auth: Vec<auth::RequiredAuth>,

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
        default_value = "squirrel",
        possible_values = &renderer::THEME_SLUGS,
        case_insensitive = true,
    )]
    color_scheme: String,

    /// Default color scheme
    #[structopt(
        short = "d",
        long = "color-scheme-dark",
        default_value = "archlinux",
        possible_values = &renderer::THEME_SLUGS,
        case_insensitive = true,
    )]
    color_scheme_dark: String,

    /// Enable QR code display
    #[structopt(short = "q", long = "qrcode")]
    qrcode: bool,

    /// Enable file uploading
    #[structopt(short = "u", long = "upload-files")]
    file_upload: bool,

    /// Enable overriding existing files during file upload
    #[structopt(short = "o", long = "overwrite-files")]
    overwrite_files: bool,

    /// Enable tar archive generation
    #[structopt(short = "r", long = "enable-tar")]
    enable_tar: bool,

    /// Enable zip archive generation
    ///
    /// WARNING: Zipping large directories can result in out-of-memory exception
    /// because zip generation is done in memory and cannot be sent on the fly
    #[structopt(short = "z", long = "enable-zip")]
    enable_zip: bool,

    /// List directories first
    #[structopt(short = "D", long = "dirs-first")]
    dirs_first: bool,

    /// Shown instead of host in page title and heading
    #[structopt(short = "t", long = "title")]
    title: Option<String>,

    /// Set custom header for responses
    #[structopt(long = "header", parse(try_from_str = parse_header), number_of_values = 1)]
    header: Vec<HeaderMap>,

    /// Hide version footer
    #[structopt(short = "F", long = "hide-version-footer")]
    hide_version_footer: bool,
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

/// Parse authentication requirement
fn parse_auth(src: &str) -> Result<auth::RequiredAuth, ContextualError> {
    let mut split = src.splitn(3, ':');
    let invalid_auth_format = Err(ContextualError::InvalidAuthFormat);

    let username = match split.next() {
        Some(username) => username,
        None => return invalid_auth_format,
    };

    // second_part is either password in username:password or method in username:method:hash
    let second_part = match split.next() {
        // This allows empty passwords, as the spec does not forbid it
        Some(password) => password,
        None => return invalid_auth_format,
    };

    let password = if let Some(hash_hex) = split.next() {
        let hash_bin = if let Ok(hash_bin) = hex::decode(hash_hex) {
            hash_bin
        } else {
            return Err(ContextualError::InvalidPasswordHash);
        };

        match second_part {
            "sha256" => auth::RequiredAuthPassword::Sha256(hash_bin),
            "sha512" => auth::RequiredAuthPassword::Sha512(hash_bin),
            _ => return Err(ContextualError::InvalidHashMethod(second_part.to_owned())),
        }
    } else {
        // To make it Windows-compatible, the password needs to be shorter than 255 characters.
        // After 255 characters, Windows will truncate the value.
        // As for the username, the spec does not mention a limit in length
        if second_part.len() > 255 {
            return Err(ContextualError::PasswordTooLongError);
        }

        auth::RequiredAuthPassword::Plain(second_part.to_owned())
    };

    Ok(auth::RequiredAuth {
        username: username.to_owned(),
        password,
    })
}

/// Custom header parser (allow multiple headers input)
pub fn parse_header(src: &str) -> Result<HeaderMap, httparse::Error> {
    let mut headers = [httparse::EMPTY_HEADER; 1];
    let mut header = src.to_string();
    header.push('\n');
    httparse::parse_headers(header.as_bytes(), &mut headers)?;

    let mut header_map = HeaderMap::new();
    if let Some(h) = headers.first() {
        if h.name != httparse::EMPTY_HEADER.name {
            header_map.insert(
                HeaderName::from_bytes(&Bytes::copy_from_slice(h.name.as_bytes())).unwrap(),
                HeaderValue::from_bytes(h.value).unwrap(),
            );
        }
    }

    Ok(header_map)
}

/// Parses the command line arguments
pub fn parse_args() -> crate::MiniserveConfig {
    let args = CliArgs::from_args();

    let interfaces = if !args.interfaces.is_empty() {
        args.interfaces
    } else {
        vec![
            IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        ]
    };

    let random_route = if args.random_route {
        Some(nanoid::nanoid!(6, &ROUTE_ALPHABET))
    } else {
        None
    };

    // Generate some random routes for the favicon and css so that they are very unlikely to conflict with
    // real files.
    let favicon_route = nanoid::nanoid!(10, &ROUTE_ALPHABET);
    let css_route = nanoid::nanoid!(10, &ROUTE_ALPHABET);

    let default_color_scheme = args.color_scheme;
    let default_color_scheme_dark = args.color_scheme_dark;

    let path_explicitly_chosen = args.path.is_some();

    let port = match args.port {
        0 => free_local_port().expect("no free ports available"),
        _ => args.port,
    };

    crate::MiniserveConfig {
        verbose: args.verbose,
        path: args.path.unwrap_or_else(|| PathBuf::from(".")),
        port,
        interfaces,
        auth: args.auth,
        path_explicitly_chosen,
        no_symlinks: args.no_symlinks,
        random_route,
        favicon_route,
        css_route,
        default_color_scheme,
        default_color_scheme_dark,
        index: args.index,
        overwrite_files: args.overwrite_files,
        show_qrcode: args.qrcode,
        file_upload: args.file_upload,
        tar_enabled: args.enable_tar,
        zip_enabled: args.enable_zip,
        dirs_first: args.dirs_first,
        title: args.title,
        header: args.header,
        hide_version_footer: args.hide_version_footer,
    }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use pretty_assertions::assert_eq;

    /// Helper function that creates a `RequiredAuth` structure
    fn create_required_auth(username: &str, password: &str, encrypt: &str) -> auth::RequiredAuth {
        use auth::*;
        use RequiredAuthPassword::*;

        let password = match encrypt {
            "plain" => Plain(password.to_owned()),
            "sha256" => Sha256(hex::decode(password.to_owned()).unwrap()),
            "sha512" => Sha512(hex::decode(password.to_owned()).unwrap()),
            _ => panic!("Unknown encryption type"),
        };

        auth::RequiredAuth {
            username: username.to_owned(),
            password,
        }
    }

    #[rstest(
        auth_string, username, password, encrypt,
        case("username:password", "username", "password", "plain"),
        case("username:sha256:abcd", "username", "abcd", "sha256"),
        case("username:sha512:abcd", "username", "abcd", "sha512")
    )]
    fn parse_auth_valid(auth_string: &str, username: &str, password: &str, encrypt: &str) {
        assert_eq!(
            parse_auth(auth_string).unwrap(),
            create_required_auth(username, password, encrypt),
        );
    }

    #[rstest(
        auth_string, err_msg,
        case(
            "foo",
            "Invalid format for credentials string. Expected username:password, username:sha256:hash or username:sha512:hash"
        ),
        case(
            "username:blahblah:abcd",
            "blahblah is not a valid hashing method. Expected sha256 or sha512"
        ),
        case(
            "username:sha256:invalid",
            "Invalid format for password hash. Expected hex code"
        ),
        case(
            "username:sha512:invalid",
            "Invalid format for password hash. Expected hex code"
        ),
    )]
    fn parse_auth_invalid(auth_string: &str, err_msg: &str) {
        let err = parse_auth(auth_string).unwrap_err();
        assert_eq!(format!("{}", err), err_msg.to_owned());
    }
}

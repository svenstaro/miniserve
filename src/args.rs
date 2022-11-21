use std::net::IpAddr;
use std::path::PathBuf;

use clap::{Parser, ValueEnum, ValueHint};
use http::header::{HeaderMap, HeaderName, HeaderValue};

use crate::auth;
use crate::errors::ContextualError;
use crate::renderer::ThemeSlug;

#[derive(ValueEnum, Clone)]
pub enum MediaType {
    Image,
    Audio,
    Video,
}

#[derive(Parser)]
#[command(name = "miniserve", author, about, version)]
pub struct CliArgs {
    /// Be verbose, includes emitting access logs
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Which path to serve
    #[arg(value_hint = ValueHint::AnyPath)]
    pub path: Option<PathBuf>,

    /// The name of a directory index file to serve, like "index.html"
    ///
    /// Normally, when miniserve serves a directory, it creates a listing for that directory.
    /// However, if a directory contains this file, miniserve will serve that file instead.
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub index: Option<PathBuf>,

    /// Activate SPA (Single Page Application) mode
    ///
    /// This will cause the file given by --index to be served for all non-existing file paths. In
    /// effect, this will serve the index file whenever a 404 would otherwise occur in order to
    /// allow the SPA router to handle the request instead.
    #[arg(long, requires = "index")]
    pub spa: bool,

    /// Port to use
    #[arg(short = 'p', long = "port", default_value = "8080")]
    pub port: u16,

    /// Interface to listen on
    #[arg(
        short = 'i',
        long = "interfaces",
        value_parser(parse_interface),
        num_args(1)
    )]
    pub interfaces: Vec<IpAddr>,

    /// Set authentication. Currently supported formats:
    /// username:password, username:sha256:hash, username:sha512:hash
    /// (e.g. joe:123, joe:sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3)
    #[arg(short = 'a', long = "auth", value_parser(parse_auth), num_args(1))]
    pub auth: Vec<auth::RequiredAuth>,

    /// Use a specific route prefix
    #[arg(long = "route-prefix")]
    pub route_prefix: Option<String>,

    /// Generate a random 6-hexdigit route
    #[arg(long = "random-route", conflicts_with("route_prefix"))]
    pub random_route: bool,

    /// Hide symlinks in listing and prevent them from being followed
    #[arg(short = 'P', long = "no-symlinks")]
    pub no_symlinks: bool,

    /// Show hidden files
    #[arg(short = 'H', long = "hidden")]
    pub hidden: bool,

    /// Default color scheme
    #[arg(
        short = 'c',
        long = "color-scheme",
        default_value = "squirrel",
        ignore_case = true
    )]
    pub color_scheme: ThemeSlug,

    /// Default color scheme
    #[arg(
        short = 'd',
        long = "color-scheme-dark",
        default_value = "archlinux",
        ignore_case = true
    )]
    pub color_scheme_dark: ThemeSlug,

    /// Enable QR code display
    #[arg(short = 'q', long = "qrcode")]
    pub qrcode: bool,

    /// Enable file uploading (and optionally specify for which directory)
    #[arg(short = 'u', long = "upload-files", value_hint = ValueHint::FilePath, num_args(0..=1), value_delimiter(','))]
    pub allowed_upload_dir: Option<Vec<PathBuf>>,

    /// Enable creating directories
    #[arg(short = 'U', long = "mkdir", requires = "allowed_upload_dir")]
    pub mkdir_enabled: bool,

    /// Specify uploadable media types
    #[arg(short = 'm', long = "media-type", requires = "allowed_upload_dir")]
    pub media_type: Option<Vec<MediaType>>,

    /// Directly specify the uploadable media type expression
    #[arg(
        short = 'M',
        long = "raw-media-type",
        requires = "allowed_upload_dir",
        conflicts_with = "media_type"
    )]
    pub media_type_raw: Option<String>,

    /// Enable overriding existing files during file upload
    #[arg(short = 'o', long = "overwrite-files")]
    pub overwrite_files: bool,

    /// Enable uncompressed tar archive generation
    #[arg(short = 'r', long = "enable-tar")]
    pub enable_tar: bool,

    /// Enable gz-compressed tar archive generation
    #[arg(short = 'g', long = "enable-tar-gz")]
    pub enable_tar_gz: bool,

    /// Enable zip archive generation
    ///
    /// WARNING: Zipping large directories can result in out-of-memory exception
    /// because zip generation is done in memory and cannot be sent on the fly
    #[arg(short = 'z', long = "enable-zip")]
    pub enable_zip: bool,

    /// List directories first
    #[arg(short = 'D', long = "dirs-first")]
    pub dirs_first: bool,

    /// Shown instead of host in page title and heading
    #[arg(short = 't', long = "title")]
    pub title: Option<String>,

    /// Set custom header for responses
    #[arg(long = "header", value_parser(parse_header), num_args(1))]
    pub header: Vec<HeaderMap>,

    /// Visualize symlinks in directory listing
    #[arg(short = 'l', long = "show-symlink-info")]
    pub show_symlink_info: bool,

    /// Hide version footer
    #[arg(short = 'F', long = "hide-version-footer")]
    pub hide_version_footer: bool,

    /// Hide theme selector
    #[arg(long = "hide-theme-selector")]
    pub hide_theme_selector: bool,

    /// If enabled, display a wget command to recursively download the current directory
    #[arg(short = 'W', long = "show-wget-footer")]
    pub show_wget_footer: bool,

    /// Generate completion file for a shell
    #[arg(long = "print-completions", value_name = "shell")]
    pub print_completions: Option<clap_complete::Shell>,

    /// Generate man page
    #[arg(long = "print-manpage")]
    pub print_manpage: bool,

    /// TLS certificate to use
    #[cfg(feature = "tls")]
    #[arg(long = "tls-cert", requires = "tls_key", value_hint = ValueHint::FilePath)]
    pub tls_cert: Option<PathBuf>,

    /// TLS private key to use
    #[cfg(feature = "tls")]
    #[arg(long = "tls-key", requires = "tls_cert", value_hint = ValueHint::FilePath)]
    pub tls_key: Option<PathBuf>,

    /// Enable README.md rendering in directories
    #[arg(long)]
    pub readme: bool,
}

/// Checks whether an interface is valid, i.e. it can be parsed into an IP address
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
        let hash_bin = hex::decode(hash_hex).map_err(|_| ContextualError::InvalidPasswordHash)?;

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
    let header = format!("{}\n", src);
    httparse::parse_headers(header.as_bytes(), &mut headers)?;

    let mut header_map = HeaderMap::new();
    if let Some(h) = headers.first() {
        if h.name != httparse::EMPTY_HEADER.name {
            header_map.insert(
                HeaderName::from_bytes(h.name.as_bytes()).unwrap(),
                HeaderValue::from_bytes(h.value).unwrap(),
            );
        }
    }

    Ok(header_map)
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

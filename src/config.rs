#[cfg(feature = "tls")]
use std::{fs::File, io::BufReader};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
};

#[cfg(feature = "tls")]
use anyhow::anyhow;
use anyhow::{Context, Result};
use http::HeaderMap;

#[cfg(feature = "tls")]
use rustls_pemfile as pemfile;

use crate::{
    args::{CliArgs, Interface, MediaType},
    auth::RequiredAuth,
    file_upload::sanitize_path,
    renderer::ThemeSlug,
};

/// Possible characters for random routes
const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[derive(Clone)]
/// Configuration of the Miniserve application
pub struct MiniserveConfig {
    /// Enable verbose mode
    pub verbose: bool,

    /// Path to be served by miniserve
    pub path: std::path::PathBuf,

    /// Port on which miniserve will be listening
    pub port: u16,

    /// IP address(es) or unix domain sockets on which miniserve will be available
    pub interfaces: Vec<Interface>,

    /// Enable HTTP basic authentication
    pub auth: Vec<RequiredAuth>,

    /// If false, miniserve will serve the current working directory
    pub path_explicitly_chosen: bool,

    /// Enable symlink resolution
    pub no_symlinks: bool,

    /// Show hidden files
    pub show_hidden: bool,

    /// Route prefix; Either empty or prefixed with slash
    pub route_prefix: String,

    /// Randomly generated favicon route
    pub favicon_route: String,

    /// Randomly generated css route
    pub css_route: String,

    /// Default color scheme
    pub default_color_scheme: ThemeSlug,

    /// Default dark mode color scheme
    pub default_color_scheme_dark: ThemeSlug,

    /// The name of a directory index file to serve, like "index.html"
    ///
    /// Normally, when miniserve serves a directory, it creates a listing for that directory.
    /// However, if a directory contains this file, miniserve will serve that file instead.
    pub index: Option<std::path::PathBuf>,

    /// Activate SPA (Single Page Application) mode
    ///
    /// This will cause the file given by `index` to be served for all non-existing file paths. In
    /// effect, this will serve the index file whenever a 404 would otherwise occur in order to
    /// allow the SPA router to handle the request instead.
    pub spa: bool,

    /// Enable QR code display
    pub show_qrcode: bool,

    /// Enable creating directories
    pub mkdir_enabled: bool,

    /// Enable file upload
    pub file_upload: bool,

    /// List of allowed upload directories
    pub allowed_upload_dir: Vec<String>,

    /// HTML accept attribute value
    pub uploadable_media_type: Option<String>,

    /// Enable upload to override existing files
    pub overwrite_files: bool,

    /// If false, creation of uncompressed tar archives is disabled
    pub tar_enabled: bool,

    /// If false, creation of gz-compressed tar archives is disabled
    pub tar_gz_enabled: bool,

    /// If false, creation of zip archives is disabled
    pub zip_enabled: bool,

    /// If enabled, directories are listed first
    pub dirs_first: bool,

    /// Shown instead of host in page title and heading
    pub title: Option<String>,

    /// If specified, header will be added
    pub header: Vec<HeaderMap>,

    /// If specified, symlink destination will be shown
    pub show_symlink_info: bool,

    /// If enabled, version footer is hidden
    pub hide_version_footer: bool,

    /// If enabled, theme selector is hidden
    pub hide_theme_selector: bool,

    /// If enabled, display a wget command to recursively download the current directory
    pub show_wget_footer: bool,

    /// If enabled, render the readme from the current directory
    pub readme: bool,

    /// If set, use provided rustls config for TLS
    #[cfg(feature = "tls")]
    pub tls_rustls_config: Option<rustls::ServerConfig>,

    #[cfg(not(feature = "tls"))]
    pub tls_rustls_config: Option<()>,
}

impl MiniserveConfig {
    /// Parses the command line arguments
    pub fn try_from_args(args: CliArgs) -> Result<Self> {
        let interfaces = if !args.interfaces.is_empty() {
            args.interfaces
        } else {
            vec![
                Interface::Address(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0))),
                Interface::Address(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            ]
        };

        let route_prefix = match (args.route_prefix, args.random_route) {
            (Some(prefix), _) => format!("/{}", prefix.trim_matches('/')),
            (_, true) => format!("/{}", nanoid::nanoid!(6, &ROUTE_ALPHABET)),
            _ => "".to_owned(),
        };

        // Generate some random routes for the favicon and css so that they are very unlikely to conflict with
        // real files.
        // If --random-route is enabled , in order to not leak the random generated route, we must not use it
        // as static files prefix.
        // Otherwise, we should apply route_prefix to static files.
        let (favicon_route, css_route) = if args.random_route {
            (
                format!("/{}", nanoid::nanoid!(10, &ROUTE_ALPHABET)),
                format!("/{}", nanoid::nanoid!(10, &ROUTE_ALPHABET)),
            )
        } else {
            (
                format!("{}/{}", route_prefix, nanoid::nanoid!(10, &ROUTE_ALPHABET)),
                format!("{}/{}", route_prefix, nanoid::nanoid!(10, &ROUTE_ALPHABET)),
            )
        };

        let default_color_scheme = args.color_scheme;
        let default_color_scheme_dark = args.color_scheme_dark;

        let path_explicitly_chosen = args.path.is_some() || args.index.is_some();

        let port = match args.port {
            0 => port_check::free_local_port().context("No free ports available")?,
            _ => args.port,
        };

        #[cfg(feature = "tls")]
        let tls_rustls_server_config =
            if let (Some(tls_cert), Some(tls_key)) = (args.tls_cert, args.tls_key) {
                let cert_file = &mut BufReader::new(
                    File::open(&tls_cert)
                        .context(format!("Couldn't access TLS certificate {tls_cert:?}"))?,
                );
                let key_file = &mut BufReader::new(
                    File::open(&tls_key).context(format!("Couldn't access TLS key {tls_key:?}"))?,
                );
                let cert_chain = pemfile::certs(cert_file).context("Reading cert file")?;
                let key = pemfile::read_all(key_file)
                    .context("Reading private key file")?
                    .into_iter()
                    .find_map(|item| match item {
                        pemfile::Item::RSAKey(key) | pemfile::Item::PKCS8Key(key) => Some(key),
                        _ => None,
                    })
                    .ok_or_else(|| anyhow!("No supported private key in file"))?;
                let server_config = rustls::ServerConfig::builder()
                    .with_safe_defaults()
                    .with_no_client_auth()
                    .with_single_cert(
                        cert_chain.into_iter().map(rustls::Certificate).collect(),
                        rustls::PrivateKey(key),
                    )?;
                Some(server_config)
            } else {
                None
            };

        #[cfg(not(feature = "tls"))]
        let tls_rustls_server_config = None;

        let uploadable_media_type = args.media_type_raw.or_else(|| {
            args.media_type.map(|types| {
                types
                    .into_iter()
                    .map(|t| match t {
                        MediaType::Audio => "audio/*",
                        MediaType::Image => "image/*",
                        MediaType::Video => "video/*",
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
        });

        Ok(MiniserveConfig {
            verbose: args.verbose,
            path: args.path.unwrap_or_else(|| PathBuf::from(".")),
            port,
            interfaces,
            auth: args.auth,
            path_explicitly_chosen,
            no_symlinks: args.no_symlinks,
            show_hidden: args.hidden,
            route_prefix,
            favicon_route,
            css_route,
            default_color_scheme,
            default_color_scheme_dark,
            index: args.index,
            spa: args.spa,
            overwrite_files: args.overwrite_files,
            show_qrcode: args.qrcode,
            mkdir_enabled: args.mkdir_enabled,
            file_upload: args.allowed_upload_dir.is_some(),
            allowed_upload_dir: args
                .allowed_upload_dir
                .unwrap_or_default()
                .iter()
                .map(|x| {
                    sanitize_path(x, false)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/")
                })
                .collect(),
            uploadable_media_type,
            tar_enabled: args.enable_tar,
            tar_gz_enabled: args.enable_tar_gz,
            zip_enabled: args.enable_zip,
            dirs_first: args.dirs_first,
            title: args.title,
            header: args.header,
            show_symlink_info: args.show_symlink_info,
            hide_version_footer: args.hide_version_footer,
            hide_theme_selector: args.hide_theme_selector,
            show_wget_footer: args.show_wget_footer,
            readme: args.readme,
            tls_rustls_config: tls_rustls_server_config,
        })
    }
}

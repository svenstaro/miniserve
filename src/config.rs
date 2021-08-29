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
use rustls::internal::pemfile::{certs, pkcs8_private_keys};

use crate::{args::CliArgs, auth::RequiredAuth};

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

    /// IP address(es) on which miniserve will be available
    pub interfaces: Vec<IpAddr>,

    /// Enable HTTP basic authentication
    pub auth: Vec<RequiredAuth>,

    /// If false, miniserve will serve the current working directory
    pub path_explicitly_chosen: bool,

    /// Enable symlink resolution
    pub no_symlinks: bool,

    /// Show hidden files
    pub show_hidden: bool,

    /// Enable random route generation
    pub random_route: Option<String>,

    /// Randomly generated favicon route
    pub favicon_route: String,

    /// Randomly generated css route
    pub css_route: String,

    /// Default color scheme
    pub default_color_scheme: String,

    /// Default dark mode color scheme
    pub default_color_scheme_dark: String,

    /// The name of a directory index file to serve, like "index.html"
    ///
    /// Normally, when miniserve serves a directory, it creates a listing for that directory.
    /// However, if a directory contains this file, miniserve will serve that file instead.
    pub index: Option<std::path::PathBuf>,

    /// Enable QR code display
    pub show_qrcode: bool,

    /// Enable file upload
    pub file_upload: bool,

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

    /// If enabled, version footer is hidden
    pub hide_version_footer: bool,

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

        let path_explicitly_chosen = args.path.is_some() || args.index.is_some();

        let port = match args.port {
            0 => port_check::free_local_port().context("No free ports available")?,
            _ => args.port,
        };

        #[cfg(feature = "tls")]
        let tls_rustls_server_config = if let (Some(tls_cert), Some(tls_key)) =
            (args.tls_cert, args.tls_key)
        {
            let mut server_config = rustls::ServerConfig::new(rustls::NoClientAuth::new());
            let cert_file = &mut BufReader::new(
                File::open(&tls_cert)
                    .context(format!("Couldn't access TLS certificate {:?}", tls_cert))?,
            );
            let key_file = &mut BufReader::new(
                File::open(&tls_key).context(format!("Couldn't access TLS key {:?}", tls_key))?,
            );
            let cert_chain = certs(cert_file).map_err(|_| anyhow!("Couldn't load certificates"))?;
            let mut keys =
                pkcs8_private_keys(key_file).map_err(|_| anyhow!("Couldn't load private key"))?;
            server_config.set_single_cert(cert_chain, keys.remove(0))?;
            Some(server_config)
        } else {
            None
        };

        #[cfg(not(feature = "tls"))]
        let tls_rustls_server_config = None;

        Ok(MiniserveConfig {
            verbose: args.verbose,
            path: args.path.unwrap_or_else(|| PathBuf::from(".")),
            port,
            interfaces,
            auth: args.auth,
            path_explicitly_chosen,
            no_symlinks: args.no_symlinks,
            show_hidden: args.hidden,
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
            tar_gz_enabled: args.enable_tar_gz,
            zip_enabled: args.enable_zip,
            dirs_first: args.dirs_first,
            title: args.title,
            header: args.header,
            hide_version_footer: args.hide_version_footer,
            tls_rustls_config: tls_rustls_server_config,
        })
    }
}

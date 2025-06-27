use std::{
    fs::File,
    io::{BufRead, BufReader},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
};

use actix_web::http::header::HeaderMap;
use anyhow::{Context, Result, anyhow};

#[cfg(feature = "tls")]
use rustls_pemfile as pemfile;

use crate::{
    args::{CliArgs, DuplicateFile, MediaType, parse_auth},
    auth::RequiredAuth,
    file_utils::sanitize_path,
    listing::{SortingMethod, SortingOrder},
    renderer::ThemeSlug,
};

/// Possible characters for random routes
const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[derive(Debug, Clone)]
/// Configuration of the Miniserve application
pub struct MiniserveConfig {
    /// Enable verbose mode
    pub verbose: bool,

    /// Path to be served by miniserve
    pub path: std::path::PathBuf,

    /// Temporary directory that should be used when files are uploaded to the server
    pub temp_upload_directory: Option<std::path::PathBuf>,

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

    /// Default sorting method
    pub default_sorting_method: SortingMethod,

    /// Default sorting order
    pub default_sorting_order: SortingOrder,

    /// Route prefix; Either empty or prefixed with slash
    pub route_prefix: String,

    /// Well-known healthcheck route (prefixed if route_prefix is provided)
    pub healthcheck_route: String,

    /// Well-known API route (prefixed if route_prefix is provided)
    pub api_route: String,

    /// Well-known favicon route (prefixed if route_prefix is provided)
    pub favicon_route: String,

    /// Well-known css route (prefixed if route_prefix is provided)
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

    /// Activate Pretty URLs mode
    ///
    /// This will cause the server to serve the equivalent `.html` file indicated by the path.
    ///
    /// `/about` will try to find `about.html` and serve it.
    pub pretty_urls: bool,

    /// Enable QR code display
    pub show_qrcode: bool,

    /// Enable recursive directory size calculation
    pub directory_size: bool,

    /// Enable creating directories
    pub mkdir_enabled: bool,

    /// Enable file upload
    pub file_upload: bool,

    /// Max amount of concurrency when uploading multiple files
    pub web_upload_concurrency: usize,

    /// List of allowed upload directories
    pub allowed_upload_dir: Vec<String>,

    /// HTML accept attribute value
    pub uploadable_media_type: Option<String>,

    /// What to do on upload if filename already exists
    pub on_duplicate_files: DuplicateFile,

    /// If false, creation of uncompressed tar archives is disabled
    pub tar_enabled: bool,

    /// If false, creation of gz-compressed tar archives is disabled
    pub tar_gz_enabled: bool,

    /// If false, creation of zip archives is disabled
    pub zip_enabled: bool,

    /// Enable  compress response
    pub compress_response: bool,

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

    /// If enabled, indexing is disabled.
    pub disable_indexing: bool,

    /// If enabled, respond to WebDAV requests (read-only).
    pub webdav_enabled: bool,

    /// If enabled, will show in exact byte size of the file
    pub show_exact_bytes: bool,

    /// If set, use provided rustls config for TLS
    #[cfg(feature = "tls")]
    pub tls_rustls_config: Option<rustls::ServerConfig>,

    #[cfg(not(feature = "tls"))]
    pub tls_rustls_config: Option<()>,

    /// Optional external URL to prepend to file links in listings
    pub file_external_url: Option<String>,
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

        let route_prefix = match (args.route_prefix, args.random_route) {
            (Some(prefix), _) => format!("/{}", prefix.trim_matches('/')),
            (_, true) => format!("/{}", nanoid::nanoid!(6, &ROUTE_ALPHABET)),
            _ => "".to_owned(),
        };

        let mut auth = args.auth;

        if let Some(path) = args.auth_file {
            let file = File::open(path)?;
            let lines = BufReader::new(file).lines();

            for line in lines {
                auth.push(parse_auth(line?.as_str())?);
            }
        }

        // Format some well-known routes at paths that are very unlikely to conflict with real
        // files.
        // If --random-route is enabled, in order to not leak the random generated route, we must not use it
        // as static files prefix.
        // Otherwise, we should apply route_prefix to static files.
        let (healthcheck_route, api_route, favicon_route, css_route) = if args.random_route {
            (
                "/__miniserve_internal/healthcheck".into(),
                "/__miniserve_internal/api".into(),
                "/__miniserve_internal/favicon.svg".into(),
                "/__miniserve_internal/style.css".into(),
            )
        } else {
            (
                format!("{}/{}", route_prefix, "__miniserve_internal/healthcheck"),
                format!("{}/{}", route_prefix, "__miniserve_internal/api"),
                format!("{}/{}", route_prefix, "__miniserve_internal/favicon.svg"),
                format!("{}/{}", route_prefix, "__miniserve_internal/style.css"),
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
                let cert_chain = pemfile::certs(cert_file)
                    .map(|cert| cert.expect("Invalid certificate in certificate chain"))
                    .collect();
                let private_key = pemfile::private_key(key_file)
                    .context("Reading private key file")?
                    .expect("No private key found");
                let server_config = rustls::ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(cert_chain, private_key)?;
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

        let allowed_upload_dir = args
            .allowed_upload_dir
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|p| {
                        sanitize_path(p, args.hidden)
                            .map(|p| p.display().to_string().replace('\\', "/"))
                            .ok_or(anyhow!("Illegal path {p:?}"))
                    })
                    .collect()
            })
            .transpose()?
            .unwrap_or_default();

        let show_exact_bytes = match args.size_display {
            crate::args::SizeDisplay::Human => false,
            crate::args::SizeDisplay::Exact => true,
        };

        Ok(Self {
            verbose: args.verbose,
            path: args.path.unwrap_or_else(|| PathBuf::from(".")),
            temp_upload_directory: args.temp_upload_directory,
            port,
            interfaces,
            auth,
            path_explicitly_chosen,
            no_symlinks: args.no_symlinks,
            show_hidden: args.hidden,
            default_sorting_method: args.default_sorting_method,
            default_sorting_order: args.default_sorting_order,
            route_prefix,
            healthcheck_route,
            api_route,
            favicon_route,
            css_route,
            default_color_scheme,
            default_color_scheme_dark,
            index: args.index,
            spa: args.spa,
            pretty_urls: args.pretty_urls,
            on_duplicate_files: args.on_duplicate_files,
            show_qrcode: args.qrcode,
            directory_size: args.directory_size,
            mkdir_enabled: args.mkdir_enabled,
            file_upload: args.allowed_upload_dir.is_some(),
            web_upload_concurrency: args.web_upload_concurrency,
            allowed_upload_dir,
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
            disable_indexing: args.disable_indexing,
            webdav_enabled: args.enable_webdav,
            tls_rustls_config: tls_rustls_server_config,
            compress_response: args.compress_response,
            show_exact_bytes,
            file_external_url: args.file_external_url,
        })
    }
}

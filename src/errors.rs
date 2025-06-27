use std::str::FromStr;

use actix_web::{
    HttpRequest, HttpResponse, ResponseError,
    body::{BoxBody, MessageBody},
    dev::{ResponseHead, ServiceRequest, ServiceResponse},
    http::{StatusCode, header},
    middleware::Next,
    web,
};
use thiserror::Error;

use crate::{MiniserveConfig, renderer::render_error};

#[derive(Debug, Error)]
pub enum StartupError {
    /// Any kind of IO errors
    #[error("{0}\ncaused by: {1}")]
    IoError(String, std::io::Error),

    /// In case miniserve was invoked without an interactive terminal and without an explicit path
    #[error("Refusing to start as no explicit serve path was set and no interactive terminal was attached
Please set an explicit serve path like: `miniserve /my/path`")]
    NoExplicitPathAndNoTerminal,

    /// In case miniserve was invoked with --no-symlinks but the serve path is a symlink
    #[error("The -P|--no-symlinks option was provided but the serve path '{0}' is a symlink")]
    NoSymlinksOptionWithSymlinkServePath(String),

    #[error("The --enable-webdav option was provided, but the serve path '{0}' is a file")]
    WebdavWithFileServePath(String),
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Any kind of IO errors
    #[error("{0}\ncaused by: {1}")]
    IoError(String, std::io::Error),

    /// Might occur during file upload, when processing the multipart request fails
    #[error("Failed to process multipart request\ncaused by: {0}")]
    MultipartError(String),

    /// Might occur during file upload
    #[error("File already exists, and the on_duplicate_files option is set to error out")]
    DuplicateFileError,

    /// Uploaded hash not correct
    #[error("File hash that was provided did not match checksum of uploaded file")]
    UploadHashMismatchError,

    /// Upload not allowed
    #[error("Upload not allowed to this directory")]
    UploadForbiddenError,

    /// Any error related to an invalid path (failed to retrieve entry name, unexpected entry type, etc)
    #[error("Invalid path\ncaused by: {0}")]
    InvalidPathError(String),

    /// Might occur if the user has insufficient permissions to create an entry in a given directory
    #[error("Insufficient permissions to create file in {0}")]
    InsufficientPermissionsError(String),

    /// Any error related to parsing
    #[error("Failed to parse {0}\ncaused by: {1}")]
    ParseError(String, String),

    /// Might occur when the creation of an archive fails
    #[error("An error occurred while creating the {0}\ncaused by: {1}")]
    ArchiveCreationError(String, Box<RuntimeError>),

    /// More specific archive creation failure reason
    #[error("{0}")]
    ArchiveCreationDetailError(String),

    /// Might occur when the HTTP credentials are not correct
    #[error("Invalid credentials for HTTP authentication")]
    InvalidHttpCredentials,

    /// Might occur when an HTTP request is invalid
    #[error("Invalid HTTP request\ncaused by: {0}")]
    InvalidHttpRequestError(String),

    /// Might occur when trying to access a page that does not exist
    #[error("Route {0} could not be found")]
    RouteNotFoundError(String),
}

impl ResponseError for RuntimeError {
    fn status_code(&self) -> StatusCode {
        use RuntimeError as E;
        use StatusCode as S;
        match self {
            E::IoError(_, _) => S::INTERNAL_SERVER_ERROR,
            E::UploadHashMismatchError => S::BAD_REQUEST,
            E::MultipartError(_) => S::BAD_REQUEST,
            E::DuplicateFileError => S::CONFLICT,
            E::UploadForbiddenError => S::FORBIDDEN,
            E::InvalidPathError(_) => S::BAD_REQUEST,
            E::InsufficientPermissionsError(_) => S::FORBIDDEN,
            E::ParseError(_, _) => S::BAD_REQUEST,
            E::ArchiveCreationError(_, err) => err.status_code(),
            E::ArchiveCreationDetailError(_) => S::INTERNAL_SERVER_ERROR,
            E::InvalidHttpCredentials => S::UNAUTHORIZED,
            E::InvalidHttpRequestError(_) => S::BAD_REQUEST,
            E::RouteNotFoundError(_) => S::NOT_FOUND,
        }
    }

    fn error_response(&self) -> HttpResponse {
        log_error_chain(self.to_string());

        let mut resp = HttpResponse::build(self.status_code());
        if let Self::InvalidHttpCredentials = self {
            resp.append_header((
                header::WWW_AUTHENTICATE,
                header::HeaderValue::from_static("Basic realm=\"miniserve\""),
            ));
        }

        resp.content_type(mime::TEXT_PLAIN_UTF_8)
            .body(self.to_string())
    }
}

/// Middleware to convert plain-text error responses to user-friendly web pages
pub async fn error_page_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let res = next.call(req).await?.map_into_boxed_body();

    if (res.status().is_client_error() || res.status().is_server_error())
        && res.request().path() != "/upload"
        && res
            .headers()
            .get(header::CONTENT_TYPE)
            .map(AsRef::as_ref)
            .and_then(|s| std::str::from_utf8(s).ok())
            .and_then(|s| mime::Mime::from_str(s).ok())
            .as_ref()
            .map(mime::Mime::essence_str)
            == Some(mime::TEXT_PLAIN.as_ref())
    {
        let req = res.request().clone();
        Ok(res.map_body(|head, body| map_error_page(&req, head, body)))
    } else {
        Ok(res)
    }
}

fn map_error_page(req: &HttpRequest, head: &mut ResponseHead, body: BoxBody) -> BoxBody {
    let error_msg = match body.try_into_bytes() {
        Ok(bytes) => bytes,
        Err(body) => return body,
    };

    let error_msg = match std::str::from_utf8(&error_msg) {
        Ok(msg) => msg,
        _ => return BoxBody::new(error_msg),
    };

    let conf = req.app_data::<web::Data<MiniserveConfig>>().unwrap();
    let return_address = req
        .headers()
        .get(header::REFERER)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("/");

    head.headers.insert(
        header::CONTENT_TYPE,
        mime::TEXT_HTML_UTF_8.essence_str().try_into().unwrap(),
    );

    BoxBody::new(render_error(error_msg, head.status, conf, return_address).into_string())
}

pub fn log_error_chain(description: String) {
    for cause in description.lines() {
        log::error!("{cause}");
    }
}

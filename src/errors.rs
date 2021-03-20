use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContextualError {
    /// Fully customized errors, not inheriting from any error
    #[error("{0}")]
    CustomError(String),

    /// Any kind of IO errors
    #[error("{0}\ncaused by: {1}")]
    IoError(String, std::io::Error),

    /// MultipartError, which might occur during file upload, when processing the multipart request fails
    #[error("Failed to process multipart request\ncaused by: {0}")]
    MultipartError(actix_multipart::MultipartError),

    /// Any error related to an invalid path (failed to retrieve entry name, unexpected entry type, etc)
    #[error("Invalid path\ncaused by: {0}")]
    InvalidPathError(String),

    /// This error might occur if the HTTP credential string does not respect the expected format
    #[error("Invalid format for credentials string. Expected username:password, username:sha256:hash or username:sha512:hash")]
    InvalidAuthFormat,

    /// This error might occure if the hash method is neither sha256 nor sha512
    #[error("{0} is not a valid hashing method. Expected sha256 or sha512")]
    InvalidHashMethod(String),

    /// This error might occur if the HTTP auth hash password is not a valid hex code
    #[error("Invalid format for password hash. Expected hex code")]
    InvalidPasswordHash,

    /// This error might occur if the HTTP auth password exceeds 255 characters
    #[error("HTTP password length exceeds 255 characters")]
    PasswordTooLongError,

    /// This error might occur if the user has unsufficient permissions to create an entry in a given directory
    #[error("Insufficient permissions to create file in {0}")]
    InsufficientPermissionsError(String),

    /// Any error related to parsing.
    #[error("Failed to parse {0}\ncaused by: {1}")]
    ParseError(String, String),

    /// This error might occur when the creation of an archive fails
    #[error("An error occured while creating the {0}\ncaused by: {1}")]
    ArchiveCreationError(String, Box<ContextualError>),

    /// This error might occur when the HTTP authentication fails
    #[error("An error occured during HTTP authentication\ncaused by: {0}")]
    HttpAuthenticationError(Box<ContextualError>),

    /// This error might occur when the HTTP credentials are not correct
    #[error("Invalid credentials for HTTP authentication")]
    InvalidHttpCredentials,

    /// This error might occur when an HTTP request is invalid
    #[error("Invalid HTTP request\ncaused by: {0}")]
    InvalidHttpRequestError(String),

    /// This error might occur when trying to access a page that does not exist
    #[error("Route {0} could not be found")]
    RouteNotFoundError(String),
}

pub fn log_error_chain(description: String) {
    for cause in description.lines() {
        log::error!("{}", cause);
    }
}

/// This makes creating CustomErrors easier
impl From<String> for ContextualError {
    fn from(msg: String) -> ContextualError {
        ContextualError::CustomError(msg)
    }
}

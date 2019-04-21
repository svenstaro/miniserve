use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Debug, Display};

#[derive(Debug, Fail)]
pub enum ContextualErrorKind {
    /// Fully customized errors, not inheriting from any error
    #[fail(display = "{}", _0)]
    CustomError(String),

    /// Any kind of IO errors
    #[fail(display = "{}\ncaused by: {}", _0, _1)]
    IOError(String, std::io::Error),

    /// MultipartError, which might occur during file upload, when processing the multipart request fails
    #[fail(display = "Failed to process multipart request\ncaused by: {}", _0)]
    MultipartError(actix_web::error::MultipartError),

    /// This error might occur when decoding the HTTP authentication header.
    #[fail(
        display = "Failed to decode HTTP authentication header\ncaused by: {}",
        _0
    )]
    Base64DecodeError(base64::DecodeError),

    /// Any error related to an invalid path (failed to retrieve entry name, unexpected entry type, etc)
    #[fail(display = "Invalid path\ncaused by: {}", _0)]
    InvalidPathError(String),

    /// This error might occur if the HTTP credential string does not respect the expected format
    #[fail(display = "Invalid format for credentials string. Expected is username:format")]
    InvalidAuthFormat,

    /// This error might occur if the HTTP auth password exceeds 255 characters
    #[fail(display = "HTTP password length exceeds 255 characters")]
    PasswordTooLongError,

    /// This error might occur if the user has unsufficient permissions to create an entry in a given directory
    #[fail(display = "Insufficient permissions to create file in {}", _0)]
    InsufficientPermissionsError(String),

    /// Any error related to parsing.
    #[fail(display = "Failed to parse {}\ncaused by: {}", _0, _1)]
    ParseError(String, String),

    /// This error might occur when the creation of an archive fails
    #[fail(
        display = "An error occured while creating the {}\ncaused by: {}",
        _0, _1
    )]
    ArchiveCreationError(String, Box<ContextualError>),
}

/// Based on https://boats.gitlab.io/failure/error-errorkind.html
pub struct ContextualError {
    inner: Context<ContextualErrorKind>,
}

impl ContextualError {
    pub fn new(kind: ContextualErrorKind) -> ContextualError {
        ContextualError {
            inner: Context::new(kind),
        }
    }
}

impl Fail for ContextualError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Debug for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl From<Context<ContextualErrorKind>> for ContextualError {
    fn from(inner: Context<ContextualErrorKind>) -> ContextualError {
        ContextualError { inner }
    }
}

impl From<ContextualErrorKind> for ContextualError {
    fn from(kind: ContextualErrorKind) -> ContextualError {
        ContextualError {
            inner: Context::new(kind),
        }
    }
}

/// This allows to create CustomErrors more simply
impl From<String> for ContextualError {
    fn from(msg: String) -> ContextualError {
        ContextualError::new(ContextualErrorKind::CustomError(msg))
    }
}

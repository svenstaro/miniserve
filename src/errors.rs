use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Debug, Display};
use yansi::Color;

/// Kinds of error which might happen during folder archive generation
#[derive(Clone, Debug, PartialEq, Eq, Fail)]
pub enum CompressionErrorKind {
    #[fail(display = "Could not open file {}", path)]
    OpenFileError { path: String },
    #[fail(display = "Could not create temporary file")]
    CreateTemporaryFileError,
    #[fail(display = "Could not create file {}", path)]
    CreateFileError { path: String },
    #[fail(display = "Could not retrieve entity name from the given path. 
        This can either mean that the entity has non UTF-8 characters in its name, 
        or that its name ends with \"..\"")]
    InvalidDirectoryName,
    #[fail(display = "Failed to create the TAR archive: {}", message)]
    TarBuildingError { message: String },
    #[fail(display = "Failed to create the GZIP archive")]
    GZipBuildingError,
    #[fail(display = "Failed to retrieve TAR content")]
    TarContentError,
    #[fail(display = "Failed to retrieve GZIP content")]
    GZipContentError,
}

pub fn print_chain(err: CompressionError) {
    for cause in Fail::iter_causes(&err) {
        println!(
            "{} {}",
            Color::Magenta.paint("Caused by:").to_string(),
            cause
        );
    }
}

pub struct CompressionError {
    inner: Context<CompressionErrorKind>,
}

impl CompressionError {
    fn new(kind: CompressionErrorKind) -> CompressionError {
        CompressionError {
            inner: Context::new(kind),
        }
    }
}

impl Fail for CompressionError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Debug for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl From<Context<CompressionErrorKind>> for CompressionError {
    fn from(inner: Context<CompressionErrorKind>) -> CompressionError {
        CompressionError { inner }
    }
}

impl From<CompressionErrorKind> for CompressionError {
    fn from(kind: CompressionErrorKind) -> CompressionError {
        CompressionError {
            inner: Context::new(kind),
        }
    }
}

impl From<std::option::NoneError> for CompressionError {
    fn from(_: std::option::NoneError) -> CompressionError {
        CompressionError::new(CompressionErrorKind::InvalidDirectoryName)
    }
}

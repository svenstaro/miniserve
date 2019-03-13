use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Debug, Display};
use yansi::{Color, Paint};

/// Kinds of errors which might happen during the generation of an archive
#[derive(Debug, Fail)]
pub enum CompressionErrorKind {
    #[fail(display = "Invalid path: directory name terminates in \"..\"")]
    InvalidDirectoryName,
    #[fail(display = "Invalid path: directory name contains invalid UTF-8 characters")]
    InvalidUTF8DirectoryName,
    #[fail(display = "Failed to create the TAR archive: {}", message)]
    TarBuildingError { message: String },
    #[fail(display = "Failed to create the GZIP archive: {}", message)]
    GZipBuildingError { message: String },
    #[fail(display = "Failed to retrieve TAR content")]
    TarContentError,
    #[fail(display = "Failed to retrieve GZIP content")]
    GZipContentError,
}

/// Prints the full chain of error, up to the root cause.
/// If RUST_BACKTRACE is set to 1, also prints the backtrace for each error
pub fn print_error_chain(err: CompressionError) {
    println!(
        "{error} {err}",
        error = Paint::red("error:").bold(),
        err = Paint::white(&err).bold()
    );
    print_backtrace(&err);
    for cause in Fail::iter_causes(&err) {
        println!(
            "{} {}",
            Color::RGB(255, 192, 0).paint("caused by:").to_string(),
            cause
        );
        print_backtrace(cause);
    }
}

/// Prints the backtrace of an error
/// RUST_BACKTRACE needs to be set to 1 to display the backtrace
fn print_backtrace(err: &dyn Fail) {
    if let Some(backtrace) = err.backtrace() {
        let backtrace = backtrace.to_string();
        if backtrace != "" {
            println!("{}", backtrace);
        }
    }
}

/// Based on https://boats.gitlab.io/failure/error-errorkind.html
pub struct CompressionError {
    inner: Context<CompressionErrorKind>,
}

impl CompressionError {
    pub fn new(kind: CompressionErrorKind) -> CompressionError {
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

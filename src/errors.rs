use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Debug, Display};

/// Kinds of errors which might happen during file upload
#[derive(Debug, Fail)]
pub enum FileUploadErrorKind {
    /// This error will occur when file overriding is off and file with same name already exists
    #[fail(display = "File with this name already exists")]
    FileExist,
    /// This error will occur when server will fail to preccess http header during file upload
    #[fail(display = "Failed to parse incoming request")]
    ParseError,
    /// This error will occur when we fail to precess multipart request
    #[fail(display = "Failed to process multipart request")]
    MultipartError(actix_web::error::MultipartError),
    /// This error may occur when trying to write incoming file to disk
    #[fail(display = "Failed to create or write to file")]
    IOError(std::io::Error),
    /// This error will occur when we he have insuffictent permissions to create new file
    #[fail(display = "Insuffitient permissions to create file")]
    InsufficientPermissions,
}

/// Kinds of errors which might happen during the generation of an archive
#[derive(Debug, Fail)]
pub enum CompressionErrorKind {
    /// This error will occur if the directory name could not be retrieved from the path
    /// See https://doc.rust-lang.org/std/path/struct.Path.html#method.file_name
    #[fail(display = "Invalid path: directory name terminates in \"..\"")]
    InvalidDirectoryName,
    /// This error will occur when trying to convert an OSString into a String, if the path
    /// contains invalid UTF-8 characters
    /// See https://doc.rust-lang.org/std/ffi/struct.OsStr.html#method.to_str
    #[fail(display = "Invalid path: directory name contains invalid UTF-8 characters")]
    InvalidUTF8DirectoryName,
    /// This error might occur while building a TAR archive, or while writing the termination sections
    /// See https://docs.rs/tar/0.4.22/tar/struct.Builder.html#method.append_dir_all
    /// and https://docs.rs/tar/0.4.22/tar/struct.Builder.html#method.into_inner
    #[fail(display = "Failed to create the TAR archive: {}", message)]
    TarBuildingError { message: String },
    /// This error might occur while building a GZIP archive, or while writing the GZIP trailer
    /// See https://docs.rs/libflate/0.1.21/libflate/gzip/struct.Encoder.html#method.finish
    #[fail(display = "Failed to create the GZIP archive: {}", message)]
    GZipBuildingError { message: String },
}

/// Prints the full chain of error, up to the root cause.
/// If RUST_BACKTRACE is set to 1, also prints the backtrace for each error
pub fn print_error_chain(err: CompressionError) {
    log::error!("{}", &err);
    print_backtrace(&err);
    for cause in Fail::iter_causes(&err) {
        log::error!("caused by: {}", cause);
        print_backtrace(cause);
    }
}

/// Prints the backtrace of an error
/// RUST_BACKTRACE needs to be set to 1 to display the backtrace
fn print_backtrace(err: &dyn Fail) {
    if let Some(backtrace) = err.backtrace() {
        let backtrace = backtrace.to_string();
        if backtrace != "" {
            log::error!("{}", backtrace);
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

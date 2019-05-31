use actix_web::http::ContentEncoding;
use bytes::Bytes;
use libflate::gzip::Encoder;
use serde::Deserialize;
use std::io;
use std::path::Path;
use strum_macros::{Display, EnumIter, EnumString};
use tar::Builder;

use crate::errors::ContextualError;

/// Available compression methods
#[derive(Deserialize, Clone, EnumIter, EnumString, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CompressionMethod {
    /// TAR GZ
    TarGz,
}

impl CompressionMethod {
    pub fn extension(&self) -> String {
        match &self {
            CompressionMethod::TarGz => "tar.gz",
        }
        .to_string()
    }

    pub fn content_type(&self) -> String {
        match &self {
            CompressionMethod::TarGz => "application/gzip",
        }
        .to_string()
    }

    pub fn content_encoding(&self) -> ContentEncoding {
        match &self {
            CompressionMethod::TarGz => ContentEncoding::Gzip,
        }
    }
    /// Creates an archive of a folder, using the algorithm the user chose from the web interface
    /// This method returns the archive as a stream of bytes
    pub fn create_archive<T: AsRef<Path>>(
        &self,
        dir: T,
        skip_symlinks: bool,
    ) -> Result<(String, Bytes), ContextualError> {
        match self {
            CompressionMethod::TarGz => tgz_compress(dir, skip_symlinks),
        }
    }
}

/// Compresses a given folder in .tar.gz format, and returns the result as a stream of bytes
fn tgz_compress<T: AsRef<Path>>(
    dir: T,
    skip_symlinks: bool,
) -> Result<(String, Bytes), ContextualError> {
    if let Some(inner_folder) = dir.as_ref().file_name() {
        if let Some(directory) = inner_folder.to_str() {
            let dst_filename = format!("{}.tar", directory);
            let dst_tgz_filename = format!("{}.gz", dst_filename);
            let mut tgz_data = Bytes::new();

            let tar_data =
                tar(dir.as_ref(), directory.to_string(), skip_symlinks).map_err(|e| {
                    ContextualError::ArchiveCreationError("tarball".to_string(), Box::new(e))
                })?;

            let gz_data = gzip(&tar_data).map_err(|e| {
                ContextualError::ArchiveCreationError("GZIP archive".to_string(), Box::new(e))
            })?;

            tgz_data.extend_from_slice(&gz_data);

            Ok((dst_tgz_filename, tgz_data))
        } else {
            // https://doc.rust-lang.org/std/ffi/struct.OsStr.html#method.to_str
            Err(ContextualError::InvalidPathError(
                "Directory name contains invalid UTF-8 characters".to_string(),
            ))
        }
    } else {
        // https://doc.rust-lang.org/std/path/struct.Path.html#method.file_name
        Err(ContextualError::InvalidPathError(
            "Directory name terminates in \"..\"".to_string(),
        ))
    }
}

/// Creates a TAR archive of a folder, and returns it as a stream of bytes
fn tar<T: AsRef<Path>>(
    src_dir: T,
    inner_folder: String,
    skip_symlinks: bool,
) -> Result<Vec<u8>, ContextualError> {
    let mut tar_builder = Builder::new(Vec::new());

    tar_builder.follow_symlinks(!skip_symlinks);
    // Recursively adds the content of src_dir into the archive stream
    tar_builder
        .append_dir_all(inner_folder, src_dir.as_ref())
        .map_err(|e| {
            ContextualError::IOError(
                format!(
                    "Failed to append the content of {} to the TAR archive",
                    src_dir.as_ref().to_str().unwrap_or("file")
                ),
                e,
            )
        })?;

    let tar_content = tar_builder.into_inner().map_err(|e| {
        ContextualError::IOError("Failed to finish writing the TAR archive".to_string(), e)
    })?;

    Ok(tar_content)
}

/// Compresses a stream of bytes using the GZIP algorithm, and returns the resulting stream
fn gzip(mut data: &[u8]) -> Result<Vec<u8>, ContextualError> {
    let mut encoder = Encoder::new(Vec::new())
        .map_err(|e| ContextualError::IOError("Failed to create GZIP encoder".to_string(), e))?;
    io::copy(&mut data, &mut encoder)
        .map_err(|e| ContextualError::IOError("Failed to write GZIP data".to_string(), e))?;
    let data = encoder
        .finish()
        .into_result()
        .map_err(|e| ContextualError::IOError("Failed to write GZIP trailer".to_string(), e))?;

    Ok(data)
}

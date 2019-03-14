use actix_web::http::ContentEncoding;
use bytes::Bytes;
use failure::ResultExt;
use libflate::gzip::Encoder;
use serde::Deserialize;
use std::io;
use std::path::PathBuf;
use tar::Builder;

use crate::errors;

/// Available compression methods
#[derive(Debug, Deserialize, Clone)]
pub enum CompressionMethod {
    /// TAR GZ
    #[serde(alias = "targz")]
    TarGz,
}

impl CompressionMethod {
    pub fn to_string(&self) -> String {
        match &self {
            CompressionMethod::TarGz => "targz",
        }
        .to_string()
    }

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
}

pub fn create_archive_file(
    method: &CompressionMethod,
    dir: &PathBuf,
    skip_symlinks: bool
) -> Result<(String, Bytes), errors::CompressionError> {
    match method {
        CompressionMethod::TarGz => tgz_compress(&dir, skip_symlinks),
    }
}

/// Compresses a given folder in .tar.gz format
fn tgz_compress(dir: &PathBuf, skip_symlinks: bool) -> Result<(String, Bytes), errors::CompressionError> {
    let src_dir = dir.display().to_string();
    let inner_folder = match dir.file_name() {
        Some(directory_name) => match directory_name.to_str() {
            Some(directory) => directory,
            None => {
                return Err(errors::CompressionError::new(
                    errors::CompressionErrorKind::InvalidUTF8DirectoryName,
                ))
            }
        },
        None => {
            return Err(errors::CompressionError::new(
                errors::CompressionErrorKind::InvalidDirectoryName,
            ))
        }
    };
    let dst_filename = format!("{}.tar", inner_folder);
    let dst_tgz_filename = format!("{}.gz", dst_filename);

    let tar_content = tar(src_dir, inner_folder.to_string(), skip_symlinks)
        .context(errors::CompressionErrorKind::TarContentError)?;
    let gz_data = gzip(&tar_content).context(errors::CompressionErrorKind::GZipContentError)?;

    let mut data = Bytes::new();
    data.extend_from_slice(&gz_data);

    Ok((dst_tgz_filename, data))
}

/// Creates a temporary tar file of a given directory, reads it and returns its content as bytes
fn tar(src_dir: String, inner_folder: String, skip_symlinks: bool) -> Result<Vec<u8>, errors::CompressionError> {
    // Create a TAR file of src_dir
    let mut tar_builder = Builder::new(Vec::new());

    tar_builder.follow_symlinks(!skip_symlinks);
    tar_builder.append_dir_all(inner_folder, &src_dir).context(
        errors::CompressionErrorKind::TarBuildingError {
            message: format!(
                "failed to append the content of {} to the TAR archive",
                &src_dir
            ),
        },
    )?;

    let tar_content =
        tar_builder
            .into_inner()
            .context(errors::CompressionErrorKind::TarBuildingError {
                message: "failed to finish writing the TAR archive".to_string(),
            })?;

    Ok(tar_content)
}

/// Compresses a stream of bytes using the GZIP algorithm
fn gzip(mut data: &[u8]) -> Result<Vec<u8>, errors::CompressionError> {
    let mut encoder =
        Encoder::new(Vec::new()).context(errors::CompressionErrorKind::GZipBuildingError {
            message: "failed to create GZIP encoder".to_string(),
        })?;
    io::copy(&mut data, &mut encoder).context(errors::CompressionErrorKind::GZipBuildingError {
        message: "failed to write GZIP data".to_string(),
    })?;
    let data = encoder.finish().into_result().context(
        errors::CompressionErrorKind::GZipBuildingError {
            message: "failed to write GZIP trailer".to_string(),
        },
    )?;

    Ok(data)
}

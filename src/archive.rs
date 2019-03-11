use actix_web::http::ContentEncoding;
use bytes::Bytes;
use failure::ResultExt;
use libflate::gzip::Encoder;
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::PathBuf;
use tar::Builder;
use tempfile::tempdir;
use yansi::Color;

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
) -> Result<(String, Bytes), errors::CompressionError> {
    match method {
        CompressionMethod::TarGz => tgz_compress(&dir),
    }
}

/// Compresses a given folder in .tar.gz format
fn tgz_compress(dir: &PathBuf) -> Result<(String, Bytes), errors::CompressionError> {
    let src_dir = dir.display().to_string();
    let inner_folder = dir.file_name()?.to_str()?;
    let dst_filename = format!("{}.tar", inner_folder);
    let dst_tgz_filename = format!("{}.gz", dst_filename);

    let tar_content = tar(src_dir, dst_filename, inner_folder.to_string())
        .context(errors::CompressionErrorKind::TarContentError)?;
    let gz_data = gzip(&tar_content).context(errors::CompressionErrorKind::GZipContentError)?;

    let mut data = Bytes::new();
    data.extend_from_slice(&gz_data);

    Ok((dst_tgz_filename, data))
}

/// Creates a temporary tar file of a given directory, reads it and returns its content as bytes
fn tar(
    src_dir: String,
    dst_filename: String,
    inner_folder: String,
) -> Result<Vec<u8>, errors::CompressionError> {
    let tmp_dir = tempdir().context(errors::CompressionErrorKind::CreateTemporaryFileError)?;
    let dst_filepath = tmp_dir.path().join(dst_filename.clone());
    let tar_file =
        File::create(&dst_filepath).context(errors::CompressionErrorKind::CreateFileError {
            path: color_path(&dst_filepath.display().to_string()),
        })?;

    // Create a TAR file of src_dir
    let mut tar_builder = Builder::new(&tar_file);

    // Temporary workaround for known issue:
    // https://github.com/alexcrichton/tar-rs/issues/147
    // https://github.com/alexcrichton/tar-rs/issues/174
    tar_builder.follow_symlinks(false);
    tar_builder.append_dir_all(inner_folder, &src_dir).context(
        errors::CompressionErrorKind::TarBuildingError {
            message: format!(
                "failed to append the content of {} in the TAR archive",
                color_path(&src_dir)
            ),
        },
    )?;
    tar_builder
        .into_inner()
        .context(errors::CompressionErrorKind::TarBuildingError {
            message: "failed to finish writing the TAR archive".to_string(),
        })?;

    // Read the content of the TAR file and store it as bytes
    let mut tar_file = OpenOptions::new().read(true).open(&dst_filepath).context(
        errors::CompressionErrorKind::OpenFileError {
            path: color_path(&dst_filepath.display().to_string()),
        },
    )?;
    let mut tar_content = Vec::new();
    tar_file
        .read_to_end(&mut tar_content)
        .context(errors::CompressionErrorKind::TarContentError)?;

    Ok(tar_content)
}

/// Compresses a stream of bytes using the GZIP algorithm
fn gzip(mut data: &[u8]) -> Result<Vec<u8>, errors::CompressionError> {
    let mut encoder =
        Encoder::new(Vec::new()).context(errors::CompressionErrorKind::GZipBuildingError)?;
    io::copy(&mut data, &mut encoder).context(errors::CompressionErrorKind::GZipBuildingError)?;
    let data = encoder
        .finish()
        .into_result()
        .context(errors::CompressionErrorKind::GZipBuildingError)?;

    Ok(data)
}

fn color_path(path: &str) -> String {
    Color::White.paint(path).bold().to_string()
}

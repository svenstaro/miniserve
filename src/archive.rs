use actix_web::http::ContentEncoding;
use bytes::Bytes;
use libflate::gzip::Encoder;
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::PathBuf;
use tar::Builder;
use tempfile::tempdir;

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

/// Possible errors
#[derive(Debug)]
pub enum CompressionError {
    IOError(std::io::Error),
    NoneError(std::option::NoneError),
}

impl From<std::option::NoneError> for CompressionError {
    fn from(err: std::option::NoneError) -> CompressionError {
        CompressionError::NoneError(err)
    }
}

impl From<std::io::Error> for CompressionError {
    fn from(err: std::io::Error) -> CompressionError {
        CompressionError::IOError(err)
    }
}

pub fn create_archive_file(
    method: &CompressionMethod,
    dir: &PathBuf,
) -> Result<(String, Bytes), CompressionError> {
    match method {
        CompressionMethod::TarGz => tgz_compress(&dir),
    }
}

/// Compresses a given folder in .tar.gz format
fn tgz_compress(dir: &PathBuf) -> Result<(String, Bytes), CompressionError> {
    let src_dir = dir.display().to_string();
    let inner_folder = dir.file_name()?.to_str()?;
    let dst_filename = format!("{}.tar", inner_folder);
    let dst_tgz_filename = format!("{}.gz", dst_filename);

    let tar_content = tar(src_dir, dst_filename, inner_folder.to_string())?;
    let gz_data = gzip(&tar_content)?;

    let mut data = Bytes::new();
    data.extend_from_slice(&gz_data);

    Ok((dst_tgz_filename, data))
}

/// Creates a temporary tar file of a given directory, reads it and returns its content as bytes
fn tar(
    src_dir: String,
    dst_filename: String,
    inner_folder: String,
) -> Result<Vec<u8>, CompressionError> {
    let tmp_dir = tempdir()?;
    let dst_filepath = tmp_dir.path().join(dst_filename.clone());
    let tar_file = File::create(&dst_filepath)?;

    // Create a TAR file of src_dir
    let mut tar_builder = Builder::new(&tar_file);

    // Temporary workaround for known issue:
    // https://github.com/alexcrichton/tar-rs/issues/147
    // https://github.com/alexcrichton/tar-rs/issues/174
    tar_builder.follow_symlinks(false);
    tar_builder.append_dir_all(inner_folder, src_dir)?;
    tar_builder.into_inner()?;

    // Read the content of the TAR file and store it as bytes
    let mut tar_file = OpenOptions::new().read(true).open(&dst_filepath)?;
    let mut tar_content = Vec::new();
    tar_file.read_to_end(&mut tar_content)?;

    Ok(tar_content)
}

/// Compresses a stream of bytes using the GZIP algorithm
fn gzip(mut data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    let mut encoder = Encoder::new(Vec::new())?;
    io::copy(&mut data, &mut encoder)?;
    let data = encoder.finish().into_result()?;

    Ok(data)
}

use bytes::Bytes;
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::PathBuf;
use tar::Builder;
use tempfile::tempdir;

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

/// Available compression methods
#[derive(Debug, Deserialize, Clone)]
pub enum CompressionMethod {
    /// ZIP
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
}

pub fn create_archive_file(
    method: &CompressionMethod,
    dir: &PathBuf,
) -> Result<(String, usize, Bytes), CompressionError> {
    match method {
        CompressionMethod::TarGz => tgz_compress(&dir),
    }
}

fn tgz_compress(dir: &PathBuf) -> Result<(String, usize, Bytes), CompressionError> {
    let src_dir = dir.display().to_string();
    let inner_folder = dir.file_name()?.to_str()?;
    let dst_filename = format!("{}.tar", inner_folder);
    let tmp_dir = tempdir()?;

    let dst_filepath = tmp_dir.path().join(dst_filename.clone());
    let tar_file = File::create(&dst_filepath)?;
    let mut tar_builder = Builder::new(&tar_file);
    tar_builder.append_dir_all(inner_folder, src_dir)?;
    tar_builder.finish()?;

    let mut tar_file = OpenOptions::new().read(true).open(&dst_filepath)?;
    let mut contents = Vec::new();
    let content_length = tar_file.read_to_end(&mut contents).unwrap();

    let mut data = Bytes::new();
    data.extend_from_slice(&contents);

    Ok((dst_filename, content_length, data))
}

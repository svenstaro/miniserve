use actix_web::http::ContentEncoding;
use libflate::gzip::Encoder;
use serde::Deserialize;
use std::path::Path;
use strum_macros::{Display, EnumIter, EnumString};
use tar::Builder;

use crate::errors::ContextualError;

/// Available compression methods
#[derive(Deserialize, Clone, Copy, EnumIter, EnumString, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CompressionMethod {
    /// Gzipped tarball
    TarGz,

    /// Regular tarball
    Tar,
}

impl CompressionMethod {
    pub fn extension(self) -> String {
        match self {
            CompressionMethod::TarGz => "tar.gz",
            CompressionMethod::Tar => "tar",
        }
        .to_string()
    }

    pub fn content_type(self) -> String {
        match self {
            CompressionMethod::TarGz => "application/gzip",
            CompressionMethod::Tar => "application/tar",
        }
        .to_string()
    }

    pub fn content_encoding(self) -> ContentEncoding {
        match self {
            CompressionMethod::TarGz => ContentEncoding::Gzip,
            CompressionMethod::Tar => ContentEncoding::Identity,
        }
    }

    pub fn create_archive<T, W>(
        self,
        dir: T,
        skip_symlinks: bool,
        out: W,
    ) -> Result<(), ContextualError>
    where
        T: AsRef<Path>,
        W: std::io::Write,
    {
        let dir = dir.as_ref();
        match self {
            CompressionMethod::TarGz => tar_gz(dir, skip_symlinks, out),
            CompressionMethod::Tar => tar_dir(dir, skip_symlinks, out),
        }
    }
}

fn tar_gz<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    let mut out = Encoder::new(out).map_err(|e| ContextualError::IOError("GZIP".to_string(), e))?;

    tar_dir(dir, skip_symlinks, &mut out)?;

    out.finish()
        .into_result()
        .map_err(|e| ContextualError::IOError("GZIP finish".to_string(), e))?;

    Ok(())
}

fn tar_dir<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    if let Some(inner_folder) = dir.file_name() {
        if let Some(directory) = inner_folder.to_str() {
            tar(dir, directory.to_string(), skip_symlinks, out).map_err(|e| {
                ContextualError::ArchiveCreationError("tarball".to_string(), Box::new(e))
            })
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

fn tar<W>(
    src_dir: &Path,
    inner_folder: String,
    skip_symlinks: bool,
    out: W,
) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    let mut tar_builder = Builder::new(out);

    tar_builder.follow_symlinks(!skip_symlinks);

    // Recursively adds the content of src_dir into the archive stream
    tar_builder
        .append_dir_all(inner_folder, src_dir)
        .map_err(|e| {
            ContextualError::IOError(
                format!(
                    "Failed to append the content of {} to the TAR archive",
                    src_dir.to_str().unwrap_or("file")
                ),
                e,
            )
        })?;

    // Finish the archive
    tar_builder.into_inner().map_err(|e| {
        ContextualError::IOError("Failed to finish writing the TAR archive".to_string(), e)
    })?;

    Ok(())
}

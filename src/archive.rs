use std::collections::HashSet;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use libflate::gzip::Encoder;
use serde::Deserialize;
use strum::{Display, EnumIter, EnumString};
use tar::Builder;
use zip::{ZipWriter, write};

use crate::errors::RuntimeError;

/// Whether `path` is equal to or nested under `root`.
///
/// Both paths are expected to already be canonicalized so that symlink
/// components and `..` segments cannot fool a prefix check.
fn is_within_root(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

/// Decide what to do with a symlink encountered while building an archive.
///
/// - When `skip_symlinks` is true, the entry is ignored entirely.
/// - When following is allowed, only targets that resolve **inside**
///   `canonical_root` are included. Targets that escape the archive root, and
///   broken links that cannot be resolved, are omitted so a single bad entry
///   cannot abort (or poison) the whole archive download.
enum SymlinkAction {
    /// Omit the entry from the archive.
    Skip,
    /// Follow the symlink; `resolved` is the canonical target path.
    Follow { resolved: PathBuf },
}

fn symlink_action(entry_path: &Path, canonical_root: &Path, skip_symlinks: bool) -> SymlinkAction {
    if skip_symlinks {
        return SymlinkAction::Skip;
    }

    match entry_path.canonicalize() {
        Ok(resolved) if is_within_root(&resolved, canonical_root) => {
            SymlinkAction::Follow { resolved }
        }
        // Outside the archive root, or unresolvable (broken link / permissions):
        // never package the target contents.
        _ => SymlinkAction::Skip,
    }
}

/// Available archive methods
#[derive(Deserialize, Clone, Copy, EnumIter, EnumString, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ArchiveMethod {
    /// Gzipped tarball
    TarGz,

    /// Regular tarball
    Tar,

    /// Regular zip
    Zip,
}

impl ArchiveMethod {
    pub fn extension(self) -> String {
        match self {
            Self::TarGz => "tar.gz",
            Self::Tar => "tar",
            Self::Zip => "zip",
        }
        .to_string()
    }

    pub fn content_type(self) -> String {
        match self {
            Self::TarGz => "application/gzip",
            Self::Tar => "application/tar",
            Self::Zip => "application/zip",
        }
        .to_string()
    }

    pub fn is_enabled(self, tar_enabled: bool, tar_gz_enabled: bool, zip_enabled: bool) -> bool {
        match self {
            Self::TarGz => tar_gz_enabled,
            Self::Tar => tar_enabled,
            Self::Zip => zip_enabled,
        }
    }

    /// Make an archive out of the given directory, and write the output to the given writer.
    ///
    /// Recursively includes all files and subdirectories.
    ///
    /// If `skip_symlinks` is `true`, symlinks will not be followed and will just be ignored.
    ///
    /// Regardless of `skip_symlinks`, symlink targets that resolve outside the
    /// directory being archived are never included as regular file content. This
    /// prevents archive downloads from exfiltrating files outside the served root
    /// via a symlink planted inside it.
    pub fn create_archive<T, W>(
        self,
        dir: T,
        skip_symlinks: bool,
        out: W,
    ) -> Result<(), RuntimeError>
    where
        T: AsRef<Path>,
        W: std::io::Write,
    {
        let dir = dir.as_ref();
        match self {
            Self::TarGz => tar_gz(dir, skip_symlinks, out),
            Self::Tar => tar_dir(dir, skip_symlinks, out),
            Self::Zip => zip_dir(dir, skip_symlinks, out),
        }
    }
}

/// Write a gzipped tarball of `dir` in `out`.
fn tar_gz<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), RuntimeError>
where
    W: std::io::Write,
{
    let mut out = Encoder::new(out).map_err(|e| RuntimeError::IoError("GZIP".to_string(), e))?;

    tar_dir(dir, skip_symlinks, &mut out)?;

    out.finish()
        .into_result()
        .map_err(|e| RuntimeError::IoError("GZIP finish".to_string(), e))?;

    Ok(())
}

/// Write a tarball of `dir` in `out`.
///
/// The target directory will be saved as a top-level directory in the archive.
///
/// For example, consider this directory structure:
///
/// ```ignore
/// a
/// └── b
///     └── c
///         ├── e
///         ├── f
///         └── g
/// ```
///
/// Making a tarball out of `"a/b/c"` will result in this archive content:
///
/// ```ignore
/// c
/// ├── e
/// ├── f
/// └── g
/// ```
fn tar_dir<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), RuntimeError>
where
    W: std::io::Write,
{
    let inner_folder = dir.file_name().ok_or_else(|| {
        RuntimeError::InvalidPathError("Directory name terminates in \"..\"".to_string())
    })?;

    let directory = inner_folder.to_str().ok_or_else(|| {
        RuntimeError::InvalidPathError(
            "Directory name contains invalid UTF-8 characters".to_string(),
        )
    })?;

    tar(dir, directory.to_string(), skip_symlinks, out)
        .map_err(|e| RuntimeError::ArchiveCreationError("tarball".to_string(), Box::new(e)))
}

/// Writes a tarball of `dir` in `out`.
///
/// The content of `src_dir` will be saved in the archive as a folder named `inner_folder`.
fn tar<W>(
    src_dir: &Path,
    inner_folder: String,
    skip_symlinks: bool,
    out: W,
) -> Result<(), RuntimeError>
where
    W: std::io::Write,
{
    let mut tar_builder = Builder::new(out);

    // We walk the tree ourselves so we can refuse symlink targets that escape
    // the archive root. Disable the builder's own following accordingly.
    tar_builder.follow_symlinks(false);

    let canonical_root = src_dir.canonicalize().map_err(|e| {
        RuntimeError::IoError(
            format!(
                "Could not resolve archive root '{}'",
                src_dir.to_string_lossy()
            ),
            e,
        )
    })?;

    let mut visited = HashSet::new();
    visited.insert(canonical_root.clone());

    append_path_to_tar(
        &mut tar_builder,
        src_dir,
        Path::new(&inner_folder),
        &canonical_root,
        skip_symlinks,
        &mut visited,
    )?;

    // Finish the archive
    tar_builder.into_inner().map_err(|e| {
        RuntimeError::IoError("Failed to finish writing the TAR archive".to_string(), e)
    })?;

    Ok(())
}

/// Recursively append `abs_path` into the tar archive under `archive_path`.
fn append_path_to_tar<W>(
    tar_builder: &mut Builder<W>,
    abs_path: &Path,
    archive_path: &Path,
    canonical_root: &Path,
    skip_symlinks: bool,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), RuntimeError>
where
    W: std::io::Write,
{
    let entries = std::fs::read_dir(abs_path).map_err(|e| {
        RuntimeError::IoError(
            format!("Could not read directory '{}'", abs_path.to_string_lossy()),
            e,
        )
    })?;

    // Ensure the directory itself exists in the archive (empty dirs, path prefix).
    tar_builder
        .append_dir(archive_path, abs_path)
        .map_err(|e| {
            RuntimeError::IoError(
                format!(
                    "Failed to append directory '{}' to the TAR archive",
                    abs_path.to_string_lossy()
                ),
                e,
            )
        })?;

    for entry in entries {
        let entry = entry
            .map_err(|e| RuntimeError::IoError("Could not read directory entry".to_string(), e))?;
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let entry_archive_path = archive_path.join(&file_name);

        // `DirEntry::file_type` does not follow symlinks.
        let file_type = entry.file_type().map_err(|e| {
            RuntimeError::IoError(
                format!(
                    "Could not get file type of '{}'",
                    entry_path.to_string_lossy()
                ),
                e,
            )
        })?;

        if file_type.is_symlink() {
            match symlink_action(&entry_path, canonical_root, skip_symlinks) {
                SymlinkAction::Skip => continue,
                SymlinkAction::Follow { resolved } => {
                    if !visited.insert(resolved) {
                        // Circular symlink within the root — skip to avoid loops.
                        continue;
                    }
                    let meta = std::fs::metadata(&entry_path).map_err(|e| {
                        RuntimeError::IoError(
                            format!(
                                "Could not get file metadata of '{}'",
                                entry_path.to_string_lossy()
                            ),
                            e,
                        )
                    })?;
                    if meta.is_file() {
                        append_file_to_tar(tar_builder, &entry_path, &entry_archive_path)?;
                    } else if meta.is_dir() {
                        append_path_to_tar(
                            tar_builder,
                            &entry_path,
                            &entry_archive_path,
                            canonical_root,
                            skip_symlinks,
                            visited,
                        )?;
                    }
                }
            }
        } else if file_type.is_dir() {
            let canonical_dir = entry_path.canonicalize().map_err(|e| {
                RuntimeError::IoError(
                    format!(
                        "Could not resolve directory '{}'",
                        entry_path.to_string_lossy()
                    ),
                    e,
                )
            })?;
            if !visited.insert(canonical_dir) {
                continue;
            }
            append_path_to_tar(
                tar_builder,
                &entry_path,
                &entry_archive_path,
                canonical_root,
                skip_symlinks,
                visited,
            )?;
        } else if file_type.is_file() {
            append_file_to_tar(tar_builder, &entry_path, &entry_archive_path)?;
        }
        // Other special files (fifo, socket, device) are intentionally omitted.
    }

    Ok(())
}

fn append_file_to_tar<W>(
    tar_builder: &mut Builder<W>,
    abs_path: &Path,
    archive_path: &Path,
) -> Result<(), RuntimeError>
where
    W: std::io::Write,
{
    let mut file = File::open(abs_path).map_err(|e| {
        RuntimeError::IoError(
            format!("Could not open file '{}'", abs_path.to_string_lossy()),
            e,
        )
    })?;
    tar_builder
        .append_file(archive_path, &mut file)
        .map_err(|e| {
            RuntimeError::IoError(
                format!(
                    "Failed to append file '{}' to the TAR archive",
                    abs_path.to_string_lossy()
                ),
                e,
            )
        })?;
    Ok(())
}

/// Write a zip of `dir` in `out`.
///
/// The target directory will be saved as a top-level directory in the archive.
///
/// For example, consider this directory structure:
///
/// ```ignore
/// a
/// └── b
///     └── c
///         ├── e
///         ├── f
///         └── g
/// ```
///
/// Making a zip out of `"a/b/c"` will result in this archive content:
///
/// ```ignore
/// c
/// ├── e
/// ├── f
/// └── g
/// ```
fn create_zip_from_directory<W>(
    out: W,
    directory: &Path,
    skip_symlinks: bool,
) -> Result<(), RuntimeError>
where
    W: std::io::Write + std::io::Seek,
{
    let options =
        write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut paths_queue: Vec<PathBuf> = vec![directory.to_path_buf()];
    let zip_root_folder_name = directory.file_name().ok_or_else(|| {
        RuntimeError::InvalidPathError("Directory name terminates in \"..\"".to_string())
    })?;

    let canonical_root = directory.canonicalize().map_err(|e| {
        RuntimeError::IoError(
            format!(
                "Could not resolve archive root '{}'",
                directory.to_string_lossy()
            ),
            e,
        )
    })?;
    let mut visited = HashSet::new();
    visited.insert(canonical_root.clone());

    let mut zip_writer = ZipWriter::new(out);
    let mut buffer = Vec::new();
    while !paths_queue.is_empty() {
        let next = paths_queue.pop().ok_or_else(|| {
            RuntimeError::ArchiveCreationDetailError("Could not get path from queue".to_string())
        })?;
        let current_dir = next.as_path();
        let directory_entry_iterator = std::fs::read_dir(current_dir)
            .map_err(|e| RuntimeError::IoError("Could not read directory".to_string(), e))?;
        let zip_directory = Path::new(zip_root_folder_name).join(
            current_dir.strip_prefix(directory).map_err(|_| {
                RuntimeError::ArchiveCreationDetailError(
                    "Could not append base directory".to_string(),
                )
            })?,
        );

        for entry in directory_entry_iterator {
            let dir_entry = entry.map_err(|e| {
                RuntimeError::IoError("Could not read directory entry".to_string(), e)
            })?;
            let entry_path = dir_entry.path();

            // `DirEntry::file_type` does not follow symlinks — unlike
            // `std::fs::metadata`, which always does. The previous check used
            // followed metadata, so `is_symlink()` was always false and
            // `--no-symlinks` was a no-op for ZIP generation (see #1568).
            let file_type = dir_entry.file_type().map_err(|e| {
                RuntimeError::IoError(
                    format!(
                        "Could not get file type of '{}'",
                        entry_path.to_string_lossy()
                    ),
                    e,
                )
            })?;

            if file_type.is_symlink() {
                match symlink_action(&entry_path, &canonical_root, skip_symlinks) {
                    SymlinkAction::Skip => continue,
                    SymlinkAction::Follow { resolved } => {
                        if !visited.insert(resolved) {
                            continue;
                        }
                    }
                }
            }

            let entry_metadata = std::fs::metadata(&entry_path).map_err(|e| {
                RuntimeError::IoError(
                    format!(
                        "Could not get file metadata of '{}'",
                        entry_path.to_string_lossy()
                    ),
                    e,
                )
            })?;

            let current_entry_name = entry_path.file_name().ok_or_else(|| {
                RuntimeError::InvalidPathError("Invalid file or directory name".to_string())
            })?;

            // To let every software correctly parse the file structure in ZIP files that are produced
            // on any platform (esp. Windows), always use forward slashes. The documentation:
            // https://users.cs.jmu.edu/buchhofp/forensics/formats/pkzip.html
            let relative_path = if cfg!(windows) {
                let branch = zip_directory
                    .as_os_str()
                    .to_string_lossy()
                    .trim_end_matches('\\') // every branch ends with two backslashes "\\".
                    .replace('\\', "/"); // every branch uses backslash "\" as path separators.
                let leaf = current_entry_name.to_string_lossy();
                format!("{branch}/{leaf}") // construct a Unix-style path in the simplest way.
            } else {
                zip_directory
                    .join(current_entry_name)
                    .into_os_string()
                    .to_string_lossy()
                    .into_owned()
            };

            if entry_metadata.is_file() {
                let mut f = File::open(&entry_path)
                    .map_err(|e| RuntimeError::IoError("Could not open file".to_string(), e))?;
                f.read_to_end(&mut buffer).map_err(|e| {
                    RuntimeError::IoError("Could not read from file".to_string(), e)
                })?;
                zip_writer.start_file(relative_path, options).map_err(|_| {
                    RuntimeError::ArchiveCreationDetailError(
                        "Could not add file path to ZIP".to_string(),
                    )
                })?;
                zip_writer.write(buffer.as_ref()).map_err(|_| {
                    RuntimeError::ArchiveCreationDetailError(
                        "Could not write file to ZIP".to_string(),
                    )
                })?;
                buffer.clear();
            } else if entry_metadata.is_dir() {
                if !file_type.is_symlink() {
                    // Track non-symlink dirs too so a later symlink can't re-enter them.
                    if let Ok(canonical_dir) = entry_path.canonicalize() {
                        visited.insert(canonical_dir);
                    }
                }
                zip_writer
                    .add_directory(relative_path, options)
                    .map_err(|_| {
                        RuntimeError::ArchiveCreationDetailError(
                            "Could not add directory path to ZIP".to_string(),
                        )
                    })?;
                paths_queue.push(entry_path.clone());
            }
        }
    }

    zip_writer.finish().map_err(|_| {
        RuntimeError::ArchiveCreationDetailError("Could not finish writing ZIP archive".to_string())
    })?;
    Ok(())
}

/// Writes a zip of `dir` in `out`.
///
/// The content of `src_dir` will be saved in the archive as the  folder named .
fn zip_data<W>(src_dir: &Path, skip_symlinks: bool, mut out: W) -> Result<(), RuntimeError>
where
    W: std::io::Write,
{
    let mut data = Vec::new();
    let memory_file = Cursor::new(&mut data);
    create_zip_from_directory(memory_file, src_dir, skip_symlinks).map_err(|e| {
        RuntimeError::ArchiveCreationError(
            "Failed to create the ZIP archive".to_string(),
            Box::new(e),
        )
    })?;

    out.write_all(data.as_mut_slice())
        .map_err(|e| RuntimeError::IoError("Failed to write the ZIP archive".to_string(), e))?;

    Ok(())
}

fn zip_dir<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), RuntimeError>
where
    W: std::io::Write,
{
    let inner_folder = dir.file_name().ok_or_else(|| {
        RuntimeError::InvalidPathError("Directory name terminates in \"..\"".to_string())
    })?;

    inner_folder.to_str().ok_or_else(|| {
        RuntimeError::InvalidPathError(
            "Directory name contains invalid UTF-8 characters".to_string(),
        )
    })?;

    zip_data(dir, skip_symlinks, out)
        .map_err(|e| RuntimeError::ArchiveCreationError("zip".to_string(), Box::new(e)))
}

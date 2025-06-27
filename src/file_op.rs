//! Handlers for file upload and removal

#[cfg(target_family = "unix")]
use std::collections::HashSet;

use std::io::ErrorKind;

#[cfg(target_family = "unix")]
use std::os::unix::fs::MetadataExt;

use std::path::{Component, Path, PathBuf};

#[cfg(target_family = "unix")]
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, http::header, web};
use async_walkdir::WalkDir;
use futures::{StreamExt, TryStreamExt};
use log::{error, info, warn};
use serde::Deserialize;
use sha2::digest::DynDigest;
use sha2::{Digest, Sha256, Sha512};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;

#[cfg(target_family = "unix")]
use tokio::sync::RwLock;

use crate::{
    args::DuplicateFile, config::MiniserveConfig, errors::RuntimeError,
    file_utils::contains_symlink, file_utils::sanitize_path,
};

enum FileHash {
    SHA256(String),
    SHA512(String),
}

impl FileHash {
    pub fn get_hasher(&self) -> Box<dyn DynDigest> {
        match self {
            Self::SHA256(_) => Box::new(Sha256::new()),
            Self::SHA512(_) => Box::new(Sha512::new()),
        }
    }

    pub fn get_hash(&self) -> &str {
        match self {
            Self::SHA256(string) => string,
            Self::SHA512(string) => string,
        }
    }
}

/// Get the recursively calculated dir size for a given dir
///
/// Counts hardlinked files only once if the OS supports hardlinks.
///
/// Expects `dir` to be sanitized. This function doesn't do any sanitization itself.
pub async fn recursive_dir_size(dir: &Path) -> Result<u64, RuntimeError> {
    #[cfg(target_family = "unix")]
    let seen_inodes = Arc::new(RwLock::new(HashSet::new()));

    let mut entries = WalkDir::new(dir);

    let mut total_size = 0;
    loop {
        match entries.next().await {
            Some(Ok(entry)) => {
                if let Ok(metadata) = entry.metadata().await
                    && metadata.is_file()
                {
                    // On Unix, we want to filter inodes that we've already seen so we get a
                    // more accurate count of real size used on disk.
                    #[cfg(target_family = "unix")]
                    {
                        let (device_id, inode) = (metadata.dev(), metadata.ino());

                        // Check if this file has been seen before based on its device ID and
                        // inode number
                        if seen_inodes.read().await.contains(&(device_id, inode)) {
                            continue;
                        } else {
                            seen_inodes.write().await.insert((device_id, inode));
                        }
                    }
                    total_size += metadata.len();
                }
            }
            Some(Err(e)) => {
                if let Some(io_err) = e.into_io() {
                    match io_err.kind() {
                        ErrorKind::PermissionDenied => warn!(
                            "Error trying to read file when calculating dir size: {io_err}, ignoring"
                        ),
                        _ => return Err(RuntimeError::InvalidPathError(io_err.to_string())),
                    }
                }
            }
            None => break,
        }
    }
    Ok(total_size)
}

/// Saves file data from a multipart form field (`field`) to `file_path`. Optionally overwriting
/// existing file and comparing the uploaded file checksum to the user provided `file_hash`.
///
/// Returns total bytes written to file.
async fn save_file(
    field: &mut actix_multipart::Field,
    mut file_path: PathBuf,
    on_duplicate_files: DuplicateFile,
    file_checksum: Option<&FileHash>,
    temporary_upload_directory: Option<&PathBuf>,
) -> Result<u64, RuntimeError> {
    if file_path.exists() {
        match on_duplicate_files {
            DuplicateFile::Error => return Err(RuntimeError::DuplicateFileError),
            DuplicateFile::Overwrite => (),
            DuplicateFile::Rename => {
                // extract extension of the file and the file stem without extension
                // file.txt => (file, txt)
                let file_name = file_path.file_stem().unwrap_or_default().to_string_lossy();
                let file_ext = file_path.extension().map(|s| s.to_string_lossy());
                for i in 1.. {
                    // increment the number N in {file_name}-{N}.{file_ext}
                    // format until available name is found (e.g. file-1.txt, file-2.txt, etc)
                    let fp = if let Some(ext) = &file_ext {
                        file_path.with_file_name(format!("{file_name}-{i}.{ext}"))
                    } else {
                        file_path.with_file_name(format!("{file_name}-{i}"))
                    };
                    // If we have a file name that doesn't exist yet then we'll use that.
                    if !fp.exists() {
                        file_path = fp;
                        break;
                    }
                }
            }
        }
    }

    let temp_upload_directory = temporary_upload_directory.cloned();
    // Tempfile doesn't support async operations, so we'll do it on a background thread.
    let temp_upload_directory_task = tokio::task::spawn_blocking(move || {
        // If the user provided a temporary directory path, then use it.
        if let Some(temp_directory) = temp_upload_directory {
            NamedTempFile::new_in(temp_directory)
        } else {
            NamedTempFile::new()
        }
    });

    // Validate that the temporary task completed successfully.
    let named_temp_file_task = match temp_upload_directory_task.await {
        Ok(named_temp_file) => Ok(named_temp_file),
        Err(err) => Err(RuntimeError::MultipartError(format!(
            "Failed to complete spawned task to create named temp file. {err}",
        ))),
    }?;

    // Validate the the temporary file was created successfully.
    let named_temp_file = match named_temp_file_task {
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Err(
            RuntimeError::InsufficientPermissionsError(file_path.display().to_string()),
        ),
        Err(err) => Err(RuntimeError::IoError(
            format!("Failed to create temporary file {}", file_path.display()),
            err,
        )),
        Ok(file) => Ok(file),
    }?;

    // Convert the temporary file into a non-temporary file. This allows us
    // to control the lifecycle of the file. This is useful for us because
    // we need to convert the temporary file into an async enabled file and
    // on successful upload, we want to move it to the target directory.
    let (file, temp_path) = named_temp_file
        .keep()
        .map_err(|err| RuntimeError::IoError("Failed to keep temporary file".into(), err.error))?;
    let mut temp_file = tokio::fs::File::from_std(file);

    let mut written_len = 0;
    let mut hasher = file_checksum.as_ref().map(|h| h.get_hasher());
    let mut save_upload_file_error: Option<RuntimeError> = None;

    // This while loop take a stream (in this case `field`) and awaits
    // new chunks from the websocket connection. The while loop reads
    // the file from the HTTP connection and writes it to disk or until
    // the stream from the multipart request is aborted.
    while let Some(Ok(bytes)) = field.next().await {
        // If the hasher exists (if the user has also sent a chunksum with the request)
        // then we want to update the hasher with the new bytes uploaded.
        if let Some(hasher) = hasher.as_mut() {
            hasher.update(&bytes)
        }
        // Write the bytes from the stream into our temporary file.
        if let Err(e) = temp_file.write_all(&bytes).await {
            // Failed to write to file. Drop it and return the error
            save_upload_file_error =
                Some(RuntimeError::IoError("Failed to write to file".into(), e));
            break;
        }
        // record the bytes written to the file.
        written_len += bytes.len() as u64;
    }

    if save_upload_file_error.is_none() {
        // Flush the changes to disk so that we are sure they are there.
        if let Err(e) = temp_file.flush().await {
            save_upload_file_error = Some(RuntimeError::IoError(
                "Failed to flush all the file writes to disk".into(),
                e,
            ));
        }
    }

    // Drop the file expcitly here because IF there is an error when writing to the
    // temp file, we won't be able to remove as per the comment in `tokio::fs::remove_file`
    // > Note that there is no guarantee that the file is immediately deleted
    // > (e.g. depending on platform, other open file descriptors may prevent immediate removal).
    drop(temp_file);

    // If there was an error during uploading.
    if let Some(e) = save_upload_file_error {
        // If there was an error when writing the file to disk, remove it and return
        // the error that was encountered.
        let _ = tokio::fs::remove_file(temp_path).await;
        return Err(e);
    }

    // There isn't a way to get notified when a request is cancelled
    // by the user in actix it seems. References:
    // - https://github.com/actix/actix-web/issues/1313
    // - https://github.com/actix/actix-web/discussions/3011
    // Therefore, we are relying on the fact that the web UI uploads a
    // hash of the file to determine if it was completed uploaded or not.
    if let Some(hasher) = hasher
        && let Some(expected_hash) = file_checksum.as_ref().map(|f| f.get_hash())
    {
        let actual_hash = hex::encode(hasher.finalize());
        if actual_hash != expected_hash {
            warn!(
                "The expected file hash {expected_hash} did not match the calculated hash of {actual_hash}. This can be caused if a file upload was aborted."
            );
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(RuntimeError::UploadHashMismatchError);
        }
    }

    info!("File upload successful to {temp_path:?}. Moving to {file_path:?}",);
    if let Err(err) = tokio::fs::rename(&temp_path, &file_path).await {
        match err.kind() {
            ErrorKind::CrossesDevices => {
                warn!(
                    "File writen to {temp_path:?} must be copied to {file_path:?} because it's on a different filesystem"
                );
                let copy_result = tokio::fs::copy(&temp_path, &file_path).await;
                if let Err(e) = tokio::fs::remove_file(&temp_path).await {
                    error!("Failed to clean up temp file at {temp_path:?} with error {e:?}");
                }
                copy_result.map_err(|e| {
                    RuntimeError::IoError(
                        format!("Failed to copy file from {temp_path:?} to {file_path:?}"),
                        e,
                    )
                })?;
            }
            _ => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(RuntimeError::IoError(
                    format!("Failed to move temporary file {temp_path:?} to {file_path:?}",),
                    err,
                ));
            }
        }
    }

    Ok(written_len)
}

struct HandleMultipartOpts<'a> {
    on_duplicate_files: DuplicateFile,
    allow_mkdir: bool,
    allow_hidden_paths: bool,
    allow_symlinks: bool,
    file_hash: Option<&'a FileHash>,
    upload_directory: Option<&'a PathBuf>,
}

/// Handles a single field in a multipart form
async fn handle_multipart(
    mut field: actix_multipart::Field,
    path: PathBuf,
    opts: HandleMultipartOpts<'_>,
) -> Result<u64, RuntimeError> {
    let HandleMultipartOpts {
        on_duplicate_files,
        allow_mkdir,
        allow_hidden_paths,
        allow_symlinks,
        file_hash,
        upload_directory,
    } = opts;
    let field_name = field.name().expect("No name field found").to_string();

    match tokio::fs::metadata(&path).await {
        Err(_) => Err(RuntimeError::InsufficientPermissionsError(
            path.display().to_string(),
        )),
        Ok(metadata) if !metadata.is_dir() => Err(RuntimeError::InvalidPathError(format!(
            "cannot upload file to {}, since it's not a directory",
            &path.display()
        ))),
        Ok(_) => Ok(()),
    }?;

    if field_name == "mkdir" {
        if !allow_mkdir {
            return Err(RuntimeError::InsufficientPermissionsError(
                path.display().to_string(),
            ));
        }

        let mut user_given_path = PathBuf::new();
        let mut absolute_path = path.clone();

        // Get the path the user gave
        let mkdir_path_bytes = field.try_next().await;
        match mkdir_path_bytes {
            Ok(Some(mkdir_path_bytes)) => {
                let mkdir_path = std::str::from_utf8(&mkdir_path_bytes).map_err(|e| {
                    RuntimeError::ParseError(
                        "Failed to parse 'mkdir' path".to_string(),
                        e.to_string(),
                    )
                })?;
                let mkdir_path = mkdir_path.replace('\\', "/");
                absolute_path.push(&mkdir_path);
                user_given_path.push(&mkdir_path);
            }
            _ => {
                return Err(RuntimeError::ParseError(
                    "Failed to parse 'mkdir' path".to_string(),
                    "".to_string(),
                ));
            }
        };

        // Disallow using `..` (parent) in mkdir path
        if user_given_path
            .components()
            .any(|c| c == Component::ParentDir)
        {
            return Err(RuntimeError::InvalidPathError(
                "Cannot use '..' in mkdir path".to_string(),
            ));
        }
        // Hidden paths check
        sanitize_path(&user_given_path, allow_hidden_paths).ok_or_else(|| {
            RuntimeError::InvalidPathError("Cannot use hidden paths in mkdir path".to_string())
        })?;

        // Ensure there are no illegal symlinks
        if !allow_symlinks {
            match contains_symlink(&absolute_path) {
                Err(err) => Err(RuntimeError::InsufficientPermissionsError(err.to_string()))?,
                Ok(true) => Err(RuntimeError::InsufficientPermissionsError(format!(
                    "{user_given_path:?} traverses through a symlink"
                )))?,
                Ok(false) => (),
            }
        }

        return match tokio::fs::create_dir_all(&absolute_path).await {
            Err(err) if err.kind() == ErrorKind::PermissionDenied => Err(
                RuntimeError::InsufficientPermissionsError(path.display().to_string()),
            ),
            Err(err) => Err(RuntimeError::IoError(
                format!("Failed to create {}", user_given_path.display()),
                err,
            )),
            Ok(_) => Ok(0),
        };
    }

    let filename = field
        .content_disposition()
        .expect("No content-disposition field found")
        .get_filename()
        .ok_or_else(|| {
            RuntimeError::ParseError(
                "HTTP header".to_string(),
                "Failed to retrieve the name of the file to upload".to_string(),
            )
        })?;

    let filename_path = sanitize_path(Path::new(&filename), allow_hidden_paths)
        .ok_or_else(|| RuntimeError::InvalidPathError("Invalid file name to upload".to_string()))?;

    // Ensure there are no illegal symlinks in the file upload path
    if !allow_symlinks {
        match contains_symlink(&path) {
            Err(err) => Err(RuntimeError::InsufficientPermissionsError(err.to_string()))?,
            Ok(true) => Err(RuntimeError::InsufficientPermissionsError(format!(
                "{path:?} traverses through a symlink"
            )))?,
            Ok(false) => (),
        }
    }

    save_file(
        &mut field,
        path.join(filename_path),
        on_duplicate_files,
        file_hash,
        upload_directory,
    )
    .await
}

/// Query parameters used by upload and rm APIs
#[derive(Deserialize, Default)]
pub struct FileOpQueryParameters {
    path: PathBuf,
}

/// Handle incoming request to upload a file or create a directory.
/// Target file path is expected as path parameter in URI and is interpreted as relative from
/// server root directory. Any path which will go outside of this directory is considered
/// invalid.
/// This method returns future.
pub async fn upload_file(
    req: HttpRequest,
    query: web::Query<FileOpQueryParameters>,
    payload: web::Payload,
) -> Result<HttpResponse, RuntimeError> {
    let conf = req.app_data::<web::Data<MiniserveConfig>>().unwrap();
    let upload_path = sanitize_path(&query.path, conf.show_hidden).ok_or_else(|| {
        RuntimeError::InvalidPathError("Invalid value for 'path' parameter".to_string())
    })?;
    let app_root_dir = conf.path.canonicalize().map_err(|e| {
        RuntimeError::IoError("Failed to resolve path served by miniserve".to_string(), e)
    })?;

    // Disallow paths outside of allowed directories
    let upload_allowed = conf.allowed_upload_dir.is_empty()
        || conf
            .allowed_upload_dir
            .iter()
            .any(|s| upload_path.starts_with(s));

    if !upload_allowed {
        return Err(RuntimeError::UploadForbiddenError);
    }

    // Disallow the target path to go outside of the served directory
    // The target directory shouldn't be canonicalized when it gets passed to
    // handle_multipart so that it can check for symlinks if needed
    let non_canonicalized_target_dir = app_root_dir.join(upload_path);
    match non_canonicalized_target_dir.canonicalize() {
        Ok(path) if !conf.no_symlinks => Ok(path),
        Ok(path) if path.starts_with(&app_root_dir) => Ok(path),
        _ => Err(RuntimeError::InvalidHttpRequestError(
            "Invalid value for 'path' parameter".to_string(),
        )),
    }?;

    let upload_directory = conf.temp_upload_directory.as_ref();

    let file_hash = if let (Some(hash), Some(hash_function)) = (
        req.headers()
            .get("X-File-Hash")
            .and_then(|h| h.to_str().ok()),
        req.headers()
            .get("X-File-Hash-Function")
            .and_then(|h| h.to_str().ok()),
    ) {
        match hash_function.to_ascii_uppercase().as_str() {
            "SHA256" => Some(FileHash::SHA256(hash.to_string())),
            "SHA512" => Some(FileHash::SHA512(hash.to_string())),
            sha => {
                return Err(RuntimeError::InvalidHttpRequestError(format!(
                    "Invalid header value found for 'X-File-Hash-Function'. Supported values are SHA256 or SHA512. Found {sha}.",
                )));
            }
        }
    } else {
        None
    };

    let hash_ref = file_hash.as_ref();
    actix_multipart::Multipart::new(req.headers(), payload)
        .map_err(|x| RuntimeError::MultipartError(x.to_string()))
        .and_then(|field| {
            handle_multipart(
                field,
                non_canonicalized_target_dir.clone(),
                HandleMultipartOpts {
                    on_duplicate_files: conf.on_duplicate_files,
                    allow_mkdir: conf.mkdir_enabled,
                    allow_hidden_paths: conf.show_hidden,
                    allow_symlinks: !conf.no_symlinks,
                    file_hash: hash_ref,
                    upload_directory,
                },
            )
        })
        .try_collect::<Vec<u64>>()
        .await?;

    let return_path = req
        .headers()
        .get(header::REFERER)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("/");

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, return_path))
        .finish())
}

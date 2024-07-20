//! Handlers for file upload and removal

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use actix_web::{http::header, web, HttpRequest, HttpResponse};
use futures::TryFutureExt;
use futures::TryStreamExt;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::{
    config::MiniserveConfig, errors::RuntimeError, file_utils::contains_symlink,
    file_utils::sanitize_path,
};

/// Saves file data from a multipart form field (`field`) to `file_path`, optionally overwriting
/// existing file.
///
/// Returns total bytes written to file.
async fn save_file(
    field: actix_multipart::Field,
    file_path: PathBuf,
    overwrite_files: bool,
) -> Result<u64, RuntimeError> {
    if !overwrite_files && file_path.exists() {
        return Err(RuntimeError::DuplicateFileError);
    }

    let file = match File::create(&file_path).await {
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Err(
            RuntimeError::InsufficientPermissionsError(file_path.display().to_string()),
        ),
        Err(err) => Err(RuntimeError::IoError(
            format!("Failed to create {}", file_path.display()),
            err,
        )),
        Ok(v) => Ok(v),
    }?;

    let (_, written_len) = field
        .map_err(|x| RuntimeError::MultipartError(x.to_string()))
        .try_fold((file, 0u64), |(mut file, written_len), bytes| async move {
            file.write_all(bytes.as_ref())
                .map_err(|e| RuntimeError::IoError("Failed to write to file".to_string(), e))
                .await?;
            Ok((file, written_len + bytes.len() as u64))
        })
        .await?;

    Ok(written_len)
}

/// Handles a single field in a multipart form
async fn handle_multipart(
    mut field: actix_multipart::Field,
    path: PathBuf,
    overwrite_files: bool,
    allow_mkdir: bool,
    allow_hidden_paths: bool,
    allow_symlinks: bool,
) -> Result<u64, RuntimeError> {
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
                ))
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

    save_file(field, path.join(filename_path), overwrite_files).await
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
    let conf = req.app_data::<MiniserveConfig>().unwrap();
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

    actix_multipart::Multipart::new(req.headers(), payload)
        .map_err(|x| RuntimeError::MultipartError(x.to_string()))
        .and_then(|field| {
            handle_multipart(
                field,
                non_canonicalized_target_dir.clone(),
                conf.overwrite_files,
                conf.mkdir_enabled,
                conf.show_hidden,
                !conf.no_symlinks,
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

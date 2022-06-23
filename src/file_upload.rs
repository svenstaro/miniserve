use actix_web::{http::header, HttpRequest, HttpResponse};
use futures::TryStreamExt;
use std::{
    io::Write,
    path::{Component, Path, PathBuf},
};

use crate::errors::ContextualError;
use crate::listing::{self};

/// Saves file data from a multipart form field (`field`) to `file_path`, optionally overwriting
/// existing file.
///
/// Returns total bytes written to file.
async fn save_file(
    field: actix_multipart::Field,
    file_path: PathBuf,
    overwrite_files: bool,
) -> Result<u64, ContextualError> {
    if !overwrite_files && file_path.exists() {
        return Err(ContextualError::DuplicateFileError);
    }

    let file = std::fs::File::create(&file_path).map_err(|e| {
        ContextualError::IoError(format!("Failed to create {}", file_path.display()), e)
    })?;

    let (_, written_len) = field
        .map_err(ContextualError::MultipartError)
        .try_fold((file, 0u64), |(mut file, written_len), bytes| async move {
            file.write_all(bytes.as_ref())
                .map_err(|e| ContextualError::IoError("Failed to write to file".to_string(), e))?;
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
) -> Result<u64, ContextualError> {
    let field_name = field.name().to_string();

    match std::fs::metadata(&path) {
        Err(_) => Err(ContextualError::InsufficientPermissionsError(
            path.display().to_string(),
        )),
        Ok(metadata) if !metadata.is_dir() => Err(ContextualError::InvalidPathError(format!(
            "cannot upload file to {}, since it's not a directory",
            &path.display()
        ))),
        Ok(metadata) if metadata.permissions().readonly() => Err(
            ContextualError::InsufficientPermissionsError(path.display().to_string()),
        ),
        Ok(_) => Ok(()),
    }?;

    if field_name == "mkdir" {
        if !allow_mkdir {
            return Err(ContextualError::InsufficientPermissionsError(
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
                    ContextualError::ParseError(
                        "Failed to parse 'mkdir' path".to_string(),
                        e.to_string(),
                    )
                })?;
                let mkdir_path = mkdir_path.replace('\\', "/");
                absolute_path.push(&mkdir_path);
                user_given_path.push(&mkdir_path);
            }
            _ => {
                return Err(ContextualError::ParseError(
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
            return Err(ContextualError::InvalidPathError(
                "Cannot use '..' in mkdir path".to_string(),
            ));
        }
        // Hidden paths check
        sanitize_path(&user_given_path, allow_hidden_paths).ok_or_else(|| {
            ContextualError::InvalidPathError("Cannot use hidden paths in mkdir path".to_string())
        })?;

        // Ensure there are no illegal symlinks
        if !allow_symlinks && contains_symlink(&absolute_path) {
            return Err(ContextualError::InsufficientPermissionsError(
                user_given_path.display().to_string(),
            ));
        }

        std::fs::create_dir_all(&absolute_path).map_err(|e| {
            ContextualError::IoError(format!("Failed to create {}", user_given_path.display()), e)
        })?;

        return Ok(0);
    }

    let filename = field.content_disposition().get_filename().ok_or_else(|| {
        ContextualError::ParseError(
            "HTTP header".to_string(),
            "Failed to retrieve the name of the file to upload".to_string(),
        )
    })?;

    let filename_path = sanitize_path(Path::new(&filename), false).ok_or_else(|| {
        ContextualError::InvalidPathError("Invalid file name to upload".to_string())
    })?;

    // Ensure there are no illegal symlinks in the file upload path
    if !allow_symlinks && contains_symlink(&path) {
        return Err(ContextualError::InsufficientPermissionsError(
            filename.to_string(),
        ));
    }

    save_file(field, path.join(filename_path), overwrite_files).await
}

/// Handle incoming request to upload a file or create a directory.
/// Target file path is expected as path parameter in URI and is interpreted as relative from
/// server root directory. Any path which will go outside of this directory is considered
/// invalid.
/// This method returns future.
pub async fn upload_file(
    req: HttpRequest,
    payload: actix_web::web::Payload,
) -> Result<HttpResponse, ContextualError> {
    let conf = req.app_data::<crate::MiniserveConfig>().unwrap();
    let return_path = if let Some(header) = req.headers().get(header::REFERER) {
        header.to_str().unwrap_or("/").to_owned()
    } else {
        "/".to_string()
    };

    let query_params = listing::extract_query_parameters(&req);
    let upload_path = query_params.path.as_ref().ok_or_else(|| {
        ContextualError::InvalidHttpRequestError("Missing query parameter 'path'".to_string())
    })?;
    let upload_path = sanitize_path(upload_path, conf.show_hidden).ok_or_else(|| {
        ContextualError::InvalidPathError("Invalid value for 'path' parameter".to_string())
    })?;

    let app_root_dir = conf.path.canonicalize().map_err(|e| {
        ContextualError::IoError("Failed to resolve path served by miniserve".to_string(), e)
    })?;

    // Disallow the target path to go outside of the served directory
    // The target directory shouldn't be canonicalized when it gets passed to
    // handle_multipart so that it can check for symlinks if needed
    let non_canonicalized_target_dir = app_root_dir.join(upload_path);
    match non_canonicalized_target_dir.canonicalize() {
        Ok(path) if !conf.no_symlinks => Ok(path),
        Ok(path) if path.starts_with(&app_root_dir) => Ok(path),
        _ => Err(ContextualError::InvalidHttpRequestError(
            "Invalid value for 'path' parameter".to_string(),
        )),
    }?;

    actix_multipart::Multipart::new(req.headers(), payload)
        .map_err(ContextualError::MultipartError)
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

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, return_path))
        .finish())
}

/// Guarantee that the path is relative and cannot traverse back to parent directories
/// and optionally prevent traversing hidden directories.
///
/// See the unit tests tests::test_sanitize_path* for examples
fn sanitize_path(path: &Path, traverse_hidden: bool) -> Option<PathBuf> {
    let mut buf = PathBuf::new();

    for comp in path.components() {
        match comp {
            Component::Normal(name) => buf.push(name),
            Component::ParentDir => {
                buf.pop();
            }
            _ => (),
        }
    }

    // Double-check that all components are Normal and check for hidden dirs
    for comp in buf.components() {
        match comp {
            Component::Normal(_) if traverse_hidden => (),
            Component::Normal(name) if !name.to_str()?.starts_with('.') => (),
            _ => return None,
        }
    }

    Some(buf)
}

/// Returns if a path goes through a symolic link
fn contains_symlink(path: &PathBuf) -> bool {
    let mut joined_path = PathBuf::new();
    for path_slice in path {
        joined_path = joined_path.join(path_slice);
        if !joined_path.exists() {
            // On Windows, `\\?\` won't exist even though it's the root
            // So, we can't just return here
            // But we don't need to check if it's a symlink since it won't be
            continue;
        }
        if joined_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case("/foo", "foo")]
    #[case("////foo", "foo")]
    #[case("C:/foo", if cfg!(windows) { "foo" } else { "C:/foo" })]
    #[case("../foo", "foo")]
    #[case("../foo/../bar/abc", "bar/abc")]
    fn test_sanitize_path(#[case] input: &str, #[case] output: &str) {
        assert_eq!(
            sanitize_path(Path::new(input), true).unwrap(),
            Path::new(output)
        );
        assert_eq!(
            sanitize_path(Path::new(input), false).unwrap(),
            Path::new(output)
        );
    }

    #[rstest]
    #[case(".foo")]
    #[case("/.foo")]
    #[case("foo/.bar/foo")]
    fn test_sanitize_path_no_hidden_files(#[case] input: &str) {
        assert_eq!(sanitize_path(Path::new(input), false), None);
    }
}

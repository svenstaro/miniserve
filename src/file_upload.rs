use actix_web::{
    dev, http::header, multipart, FromRequest, FutureResponse, HttpMessage, HttpRequest,
    HttpResponse, Query,
};
use futures::{future, future::FutureResult, Future, Stream};
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    path::{Component, PathBuf},
};

use crate::errors::{self, ContextualErrorKind};
use crate::renderer;

/// Query parameters
#[derive(Debug, Deserialize)]
struct QueryParameters {
    path: PathBuf,
}

/// Create future to save file.
fn save_file(
    field: multipart::Field<dev::Payload>,
    file_path: PathBuf,
    overwrite_files: bool,
) -> Box<Future<Item = i64, Error = ContextualErrorKind>> {
    if !overwrite_files && file_path.exists() {
        return Box::new(future::err(ContextualErrorKind::CustomError(
            "File already exists, and the overwrite_files option has not been set".to_string(),
        )));
    }

    let mut file = match std::fs::File::create(&file_path) {
        Ok(file) => file,
        Err(e) => {
            return Box::new(future::err(ContextualErrorKind::IOError(
                format!("Failed to create file in {}", file_path.display()),
                e,
            )));
        }
    };
    Box::new(
        field
            .map_err(ContextualErrorKind::MultipartError)
            .fold(0i64, move |acc, bytes| {
                let rt = file
                    .write_all(bytes.as_ref())
                    .map(|_| acc + bytes.len() as i64)
                    .map_err(|e| {
                        ContextualErrorKind::IOError("Failed to write to file".to_string(), e)
                    });
                future::result(rt)
            }),
    )
}

/// Create new future to handle file as multipart data.
fn handle_multipart(
    item: multipart::MultipartItem<dev::Payload>,
    mut file_path: PathBuf,
    overwrite_files: bool,
) -> Box<Stream<Item = i64, Error = ContextualErrorKind>> {
    match item {
        multipart::MultipartItem::Field(field) => {
            let filename = field
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .ok_or(ContextualErrorKind::ParseError)
                .and_then(|cd| {
                    header::ContentDisposition::from_raw(cd)
                        .map_err(|_| ContextualErrorKind::ParseError)
                })
                .and_then(|content_disposition| {
                    content_disposition
                        .get_filename()
                        .ok_or(ContextualErrorKind::ParseError)
                        .map(String::from)
                });
            let err = |e: ContextualErrorKind| Box::new(future::err(e).into_stream());
            match filename {
                Ok(f) => {
                    match fs::metadata(&file_path) {
                        Ok(metadata) => {
                            if !metadata.is_dir() {
                                return err(ContextualErrorKind::InvalidPathError(format!(
                                    "cannot upload file to {}, since it's not a directory",
                                    &file_path.display()
                                )));
                            } else if metadata.permissions().readonly() {
                                return err(ContextualErrorKind::InsufficientPermissionsError(
                                    file_path.display().to_string(),
                                ));
                            }
                        }
                        Err(_) => {
                            return err(ContextualErrorKind::InsufficientPermissionsError(
                                file_path.display().to_string(),
                            ));
                        }
                    }
                    file_path = file_path.join(f);
                    Box::new(save_file(field, file_path, overwrite_files).into_stream())
                }
                Err(e) => err(e(
                    "HTTP header".to_string(),
                    "Failed to retrieve the name of the file to upload".to_string(),
                )),
            }
        }
        multipart::MultipartItem::Nested(mp) => Box::new(
            mp.map_err(ContextualErrorKind::MultipartError)
                .map(move |item| handle_multipart(item, file_path.clone(), overwrite_files))
                .flatten(),
        ),
    }
}

/// Handle incoming request to upload file.
/// Target file path is expected as path parameter in URI and is interpreted as relative from
/// server root directory. Any path which will go outside of this directory is considered
/// invalid.
/// This method returns future.
pub fn upload_file(req: &HttpRequest<crate::MiniserveConfig>) -> FutureResponse<HttpResponse> {
    let return_path = if let Some(header) = req.headers().get(header::REFERER) {
        header.to_str().unwrap_or("/").to_owned()
    } else {
        "/".to_string()
    };
    let app_root_dir = if let Ok(dir) = req.state().path.canonicalize() {
        dir
    } else {
        return Box::new(create_error_response("Internal server error", &return_path));
    };
    let path = match Query::<QueryParameters>::extract(req) {
        Ok(query) => {
            if let Ok(stripped_path) = query.path.strip_prefix(Component::RootDir) {
                stripped_path.to_owned()
            } else {
                query.path.clone()
            }
        }
        Err(_) => {
            return Box::new(create_error_response(
                "Unspecified parameter path",
                &return_path,
            ))
        }
    };

    // If the target path is under the app root directory, save the file.
    let target_dir = match &app_root_dir.clone().join(path).canonicalize() {
        Ok(path) if path.starts_with(&app_root_dir) => path.clone(),
        _ => return Box::new(create_error_response("Invalid path", &return_path)),
    };
    let overwrite_files = req.state().overwrite_files;
    Box::new(
        req.multipart()
            .map_err(ContextualErrorKind::MultipartError)
            .map(move |item| handle_multipart(item, target_dir.clone(), overwrite_files))
            .flatten()
            .collect()
            .then(move |e| match e {
                Ok(_) => future::ok(
                    HttpResponse::SeeOther()
                        .header(header::LOCATION, return_path.to_string())
                        .finish(),
                ),
                Err(e) => create_error_response(&e.to_string(), &return_path),
            }),
    )
}

/// Convenience method for creating response errors, if file upload fails.
fn create_error_response(
    description: &str,
    return_path: &str,
) -> FutureResult<HttpResponse, actix_web::error::Error> {
    errors::log_error_chain(description.to_string());
    future::ok(
        HttpResponse::BadRequest()
            .content_type("text/html; charset=utf-8")
            .body(renderer::render_error(description, return_path).into_string()),
    )
}

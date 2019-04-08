use crate::errors::FileUploadErrorKind;
use crate::renderer::file_upload_error;
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
) -> Box<Future<Item = i64, Error = FileUploadErrorKind>> {
    if !overwrite_files && file_path.exists() {
        return Box::new(future::err(FileUploadErrorKind::FileExist));
    }
    let mut file = match std::fs::File::create(file_path) {
        Ok(file) => file,
        Err(e) => {
            return Box::new(future::err(FileUploadErrorKind::IOError(e)));
        }
    };
    Box::new(
        field
            .map_err(FileUploadErrorKind::MultipartError)
            .fold(0i64, move |acc, bytes| {
                let rt = file
                    .write_all(bytes.as_ref())
                    .map(|_| acc + bytes.len() as i64)
                    .map_err(FileUploadErrorKind::IOError);
                future::result(rt)
            }),
    )
}

/// Create new future to handle file as multipart data.
fn handle_multipart(
    item: multipart::MultipartItem<dev::Payload>,
    mut file_path: PathBuf,
    overwrite_files: bool,
) -> Box<Stream<Item = i64, Error = FileUploadErrorKind>> {
    match item {
        multipart::MultipartItem::Field(field) => {
            let filename = field
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .ok_or(FileUploadErrorKind::ParseError)
                .and_then(|cd| {
                    header::ContentDisposition::from_raw(cd)
                        .map_err(|_| FileUploadErrorKind::ParseError)
                })
                .and_then(|content_disposition| {
                    content_disposition
                        .get_filename()
                        .ok_or(FileUploadErrorKind::ParseError)
                        .map(String::from)
                });
            let err = |e: FileUploadErrorKind| Box::new(future::err(e).into_stream());
            match filename {
                Ok(f) => {
                    match fs::metadata(&file_path) {
                        Ok(metadata) => {
                            if !metadata.is_dir() || metadata.permissions().readonly() {
                                return err(FileUploadErrorKind::InsufficientPermissions);
                            }
                        }
                        Err(_) => {
                            return err(FileUploadErrorKind::InsufficientPermissions);
                        }
                    }
                    file_path = file_path.join(f);
                    Box::new(save_file(field, file_path, overwrite_files).into_stream())
                }
                Err(e) => err(e),
            }
        }
        multipart::MultipartItem::Nested(mp) => Box::new(
            mp.map_err(FileUploadErrorKind::MultipartError)
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
    let return_path: String = req.headers()[header::REFERER]
        .to_str()
        .unwrap_or("/")
        .to_owned();
    let app_root_dir = match req.state().path.canonicalize() {
        Ok(path) => path,
        Err(_) => return Box::new(create_error_response("Internal server error", &return_path)),
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
            .map_err(FileUploadErrorKind::MultipartError)
            .map(move |item| handle_multipart(item, target_dir.clone(), overwrite_files))
            .flatten()
            .collect()
            .then(move |e| match e {
                Ok(_) => future::ok(
                    HttpResponse::SeeOther()
                        .header(header::LOCATION, return_path.to_string())
                        .finish(),
                ),
                Err(e) => {
                    let error_description = format!("{}", e);
                    create_error_response(&error_description, &return_path)
                }
            }),
    )
}

// Convenience method for creating response errors, when file upload fails.
fn create_error_response(
    description: &str,
    return_path: &str,
) -> FutureResult<HttpResponse, actix_web::error::Error> {
    future::ok(
        HttpResponse::NotAcceptable()
            .content_type("text/html; charset=utf-8")
            .body(file_upload_error(description, return_path).into_string()),
    )
}

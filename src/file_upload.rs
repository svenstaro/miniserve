use crate::errors::FileUploadErrorKind;
use crate::renderer::file_upload_error;
use actix_web::{
    dev, http::header, multipart, FromRequest, FutureResponse, HttpMessage, HttpRequest,
    HttpResponse, Query,
};
use futures::{future, Future, Stream};
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
    override_files: bool,
) -> Box<Future<Item = i64, Error = FileUploadErrorKind>> {
    if !override_files && file_path.exists() {
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
            .map_err(|e| FileUploadErrorKind::MultipartError(e))
            .fold(0i64, move |acc, bytes| {
                let rt = file
                    .write_all(bytes.as_ref())
                    .map(|_| acc + bytes.len() as i64)
                    .map_err(|e| FileUploadErrorKind::IOError(e));
                future::result(rt)
            }),
    )
}

/// Create new future to handle file as multipart data.
fn handle_multipart(
    item: multipart::MultipartItem<dev::Payload>,
    mut file_path: PathBuf,
    override_files: bool,
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
                        .map(|cd| String::from(cd))
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
                    Box::new(save_file(field, file_path, override_files).into_stream())
                }
                Err(e) => err(e),
            }
        }
        multipart::MultipartItem::Nested(mp) => Box::new(
            mp.map_err(|e| FileUploadErrorKind::MultipartError(e))
                .map(move |item| handle_multipart(item, file_path.clone(), override_files))
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
    let app_root_dir = req.state().path.clone().canonicalize().unwrap();
    let path = match Query::<QueryParameters>::extract(req) {
        Ok(query) => {
            if let Ok(stripped_path) = query.path.strip_prefix(Component::RootDir) {
                stripped_path.to_owned()
            } else {
                query.path.clone()
            }
        }
        Err(_) => {
            return Box::new(future::ok(
                HttpResponse::BadRequest().body("Unspecified parameter path"),
            ))
        }
    };
    // this is really ugly I will try to think about something smarter
    let return_path: String = req.headers()[header::REFERER].clone().to_str().unwrap_or("/").to_owned();
    let r_p2 = return_path.clone();

    // if target path is under app root directory save file
    let target_dir = match &app_root_dir.clone().join(path.clone()).canonicalize() {
        Ok(path) if path.starts_with(&app_root_dir) => path.clone(),
        _ => return Box::new(future::ok(HttpResponse::BadRequest().body("Invalid path"))),
    };
    let override_files = req.state().override_files;
    Box::new(
        req.multipart()
            .map_err(|e| FileUploadErrorKind::MultipartError(e))
            .map(move |item| handle_multipart(item, target_dir.clone(), override_files))
            .flatten()
            .collect()
            .map(move |_| {
                HttpResponse::TemporaryRedirect()
                    .header(
                        header::LOCATION,
                        format!("{}", return_path.clone()),
                    )
                    .finish()
            })
            .or_else(move |e| {
                let error_description = format!("{}",e);
                future::ok(HttpResponse::BadRequest().body(file_upload_error(&error_description, &r_p2.clone()).into_string()))
    )
}

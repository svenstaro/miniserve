use actix_web::{
    dev, error,
    http::header::{ContentDisposition, LOCATION},
    multipart, Error, FromRequest, FutureResponse, HttpMessage, HttpRequest, HttpResponse, Query,
};
use futures::{future, Future, Stream};
use serde::Deserialize;
use std::{
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
) -> Box<Future<Item = i64, Error = Error>> {
    if !override_files && file_path.exists() {
        return Box::new(future::err(error::ErrorInternalServerError("file exists")));
    }
    let mut file = match std::fs::File::create(file_path) {
        Ok(file) => file,
        Err(e) => return Box::new(future::err(error::ErrorInternalServerError(e))),
    };
    Box::new(
        field
            .fold(0i64, move |acc, bytes| {
                let rt = file
                    .write_all(bytes.as_ref())
                    .map(|_| acc + bytes.len() as i64)
                    .map_err(|e| error::MultipartError::Payload(error::PayloadError::Io(e)));
                future::result(rt)
            })
            .map_err(|e| error::ErrorInternalServerError(e)),
    )
}

/// Create new future to handle file as multipart data.
fn handle_multipart(
    item: multipart::MultipartItem<dev::Payload>,
    mut file_path: PathBuf,
    override_files: bool,
) -> Box<Stream<Item = i64, Error = Error>> {
    match item {
        multipart::MultipartItem::Field(field) => {
            let err = || Box::new(future::err(error::ContentTypeError::ParseError.into()));
            let filename = field
                .headers()
                .get("content-disposition")
                .ok_or(err())
                .and_then(|cd| ContentDisposition::from_raw(cd).map_err(|_| err()))
                .and_then(|content_disposition| {
                    content_disposition
                        .get_filename()
                        .ok_or(err())
                        .map(|cd| String::from(cd))
                });
            match filename {
                Ok(f) => {
                    file_path = file_path.join(f);
                    Box::new(save_file(field, file_path, override_files).into_stream())
                }
                Err(e) => Box::new(e.into_stream()),
            }
        }
        multipart::MultipartItem::Nested(mp) => Box::new(
            mp.map_err(error::ErrorInternalServerError)
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

    // if target path is under app root directory save file
    let target_dir = match &app_root_dir.clone().join(path.clone()).canonicalize() {
        Ok(path) if path.starts_with(&app_root_dir) => path.clone(),
        _ => return Box::new(future::ok(HttpResponse::BadRequest().body("Invalid path"))),
    };
    let override_files = req.state().override_files;
    Box::new(
        req.multipart()
            .map_err(error::ErrorInternalServerError)
            .map(move |item| handle_multipart(item, target_dir.clone(), override_files))
            .flatten()
            .collect()
            .map(move |_| {
                HttpResponse::TemporaryRedirect()
                    .header(LOCATION, format!("{}", path.display()))
                    .finish()
            })
            .map_err(|e| e),
    )
}

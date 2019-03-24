use actix_web::{
    dev, error,
    http::header::{ContentDisposition, LOCATION},
    multipart, Error, FutureResponse, HttpMessage, HttpRequest, HttpResponse,
};
use std::io::Write;
use std::path::{Component, PathBuf};

use futures::future;
use futures::{Future, Stream};

pub fn save_file(
    field: multipart::Field<dev::Payload>,
    file_path: PathBuf,
) -> Box<Future<Item = i64, Error = Error>> {
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

pub fn handle_multipart(
    item: multipart::MultipartItem<dev::Payload>,
    mut file_path: PathBuf,
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
                    // TODO should I allow overriding existing files?
                    Box::new(save_file(field, file_path).into_stream())
                }
                Err(e) => Box::new(e.into_stream()),
            }
        }
        multipart::MultipartItem::Nested(mp) => Box::new(
            mp.map_err(error::ErrorInternalServerError)
                .map(move |item| handle_multipart(item, file_path.clone()))
                .flatten(),
        ),
    }
}

pub fn upload_file(req: &HttpRequest<crate::MiniserveConfig>) -> FutureResponse<HttpResponse> {
    if req.query().contains_key("path") {
        let path_str = req.query()["path"].clone();
        let mut path = PathBuf::from(path_str.clone());
        // serever root path should be valid
        let app_root_dir = req.state().path.clone().canonicalize().unwrap();
        // allow file upload only under current dir
        if path.has_root() {
            path = match path.strip_prefix(Component::RootDir) {
                Ok(dir) => dir.to_path_buf(),
                Err(_) => {
                    return Box::new(future::ok(HttpResponse::BadRequest().body("Invalid path")))
                }
            }
        }
        let target_dir = match app_root_dir.clone().join(path).canonicalize() {
            Ok(path) => {
                if path.starts_with(&app_root_dir) {
                    path
                } else {
                    return Box::new(future::ok(HttpResponse::BadRequest().body("Invalid path")));
                }
            }
            Err(_) => return Box::new(future::ok(HttpResponse::BadRequest().body("Invalid path"))),
        };
        if let Ok(target_path) = target_dir.canonicalize() {
            Box::new(
                req.multipart()
                    .map_err(error::ErrorInternalServerError)
                    .map(move |item| handle_multipart(item, target_path.clone()))
                    .flatten()
                    .collect()
                    .map(|_| {
                        HttpResponse::TemporaryRedirect()
                            .header(LOCATION, path_str)
                            .finish()
                    })
                    .map_err(|e| e),
            )
        } else {
            Box::new(future::ok(HttpResponse::BadRequest().body("invalid path")))
        }
    } else {
        Box::new(future::ok(HttpResponse::BadRequest().body("")))
    }
}

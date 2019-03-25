use actix_web::{
    dev, error,
    http::header::{ContentDisposition, LOCATION},
    multipart, Error, FutureResponse, HttpMessage, HttpRequest, HttpResponse,
};
use futures::{future, Future, Stream};
use std::{
    io::Write,
    path::{Component, PathBuf},
};

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

/// Handle incoming request to upload file.
/// Target file path is expected as path parameter in URI and is interpreted as relative from
/// server root directory. Any path whitch will go outside of this directory is considered
/// invalid.
/// This method returns future.
pub fn upload_file(req: &HttpRequest<crate::MiniserveConfig>) -> FutureResponse<HttpResponse> {
    if !req.query().contains_key("path") {
        return Box::new(future::ok(
            HttpResponse::BadRequest().body("Unspecified parameter path"),
        ));
    }
    // server root path should be valid so we can unwrap it
    let app_root_dir = req.state().path.clone().canonicalize().unwrap();

    let path_str = req.query()["path"].clone();
    let mut path = PathBuf::from(path_str.clone());
    if let Ok(stripped_path) = path.strip_prefix(Component::RootDir) {
        path = stripped_path.to_owned();
    }

    // if target path is under app root directory save file
    let target_dir = match &app_root_dir.clone().join(path).canonicalize() {
        Ok(path) if path.starts_with(&app_root_dir) => path.clone(),
        _ => return Box::new(future::ok(HttpResponse::BadRequest().body("Invalid path"))),
    };
    Box::new(
        req.multipart()
            .map_err(error::ErrorInternalServerError)
            .map(move |item| handle_multipart(item, target_dir.clone()))
            .flatten()
            .collect()
            .map(|_| {
                HttpResponse::TemporaryRedirect()
                    .header(LOCATION, path_str)
                    .finish()
            })
            .map_err(|e| e),
    )
}

use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, Responder, http::header::ContentType, web};
use actix_web_lab::sse;
use bytesize::ByteSize;
use dav_server::{
    DavConfig, DavHandler,
    actix::{DavRequest, DavResponse},
};
use log::{error, info, warn};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::{config::MiniserveConfig, errors::RuntimeError};
use crate::{file_op::recursive_dir_size, file_utils};

pub async fn dav_handler(req: DavRequest, davhandler: web::Data<DavHandler>) -> DavResponse {
    if let Some(prefix) = req.prefix() {
        let config = DavConfig::new().strip_prefix(prefix);
        davhandler.handle_with(config, req.request).await.into()
    } else {
        davhandler.handle(req.request).await.into()
    }
}

pub async fn error_404(req: HttpRequest) -> Result<HttpResponse, RuntimeError> {
    Err(RuntimeError::RouteNotFoundError(req.path().to_string()))
}

pub async fn healthcheck() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[derive(Deserialize, Debug)]
pub enum ApiCommand {
    /// Request the size of a particular directory
    CalculateDirSizes(Vec<String>),
}

pub type DirSizeJoinSet = JoinSet<Result<DirSize, RuntimeError>>;

// Holds the result of a calculated dir size
#[derive(Debug, Clone)]
pub struct DirSize {
    /// The web path of the dir (not the filesystem path)
    pub web_path: String,

    /// The calculcated recursive size of the dir
    pub size: u64,
}

// Reply for a calculated dir size
#[derive(Debug, Clone, Serialize)]
pub struct DirSizeReply {
    /// The web path of the dir (not the filesystem path)
    pub web_path: String,

    /// The formatted size of the dir
    pub size: String,
}

// Reply to check whether the client is still connected
//
// If the client has disconnected, we can cancel all the tasks and save some compute.
#[derive(Debug, Clone, Serialize)]
pub struct HeartbeatReply;

/// SSE API route that yields an event stream that clients can subscribe to
pub async fn api_sse(
    config: web::Data<MiniserveConfig>,
    task_joinset: web::Data<Mutex<DirSizeJoinSet>>,
) -> impl Responder {
    let (sender, receiver) = tokio::sync::mpsc::channel(2);

    actix_web::rt::spawn(async move {
        loop {
            let msg = match task_joinset.lock().await.try_join_next() {
                Some(Ok(Ok(finished_task))) => {
                    let dir_size = if config.show_exact_bytes {
                        format!("{} B", finished_task.size)
                    } else {
                        ByteSize::b(finished_task.size).to_string()
                    };

                    let dir_size_reply = DirSizeReply {
                        web_path: finished_task.web_path,
                        size: dir_size,
                    };

                    sse::Data::new_json(dir_size_reply)
                        .expect("Couldn't serialize as JSON")
                        .event("dir-size")
                }
                Some(Ok(Err(e))) => {
                    error!("Some error during dir size calculation: {e}");
                    break;
                }
                Some(Err(e)) => {
                    error!("Some error during dir size calculation joining: {e}");
                    break;
                }
                None => sse::Data::new_json(HeartbeatReply)
                    .expect("Couldn't serialize as JSON")
                    .event("heartbeat"),
            };

            if sender.send(msg.into()).await.is_err() {
                warn!("Client disconnected; could not send SSE message");
                break;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    sse::Sse::from_infallible_receiver(receiver).with_keep_alive(Duration::from_secs(3))
}

async fn handle_dir_size_tasks(
    dirs: Vec<String>,
    config: &MiniserveConfig,
    task_joinset: web::Data<Mutex<DirSizeJoinSet>>,
) -> Result<(), RuntimeError> {
    for dir in dirs {
        // The dir argument might be percent-encoded so let's decode it just in case.
        let decoded_path = percent_decode_str(&dir)
            .decode_utf8()
            .map_err(|e| RuntimeError::ParseError(dir.clone(), e.to_string()))?;

        // Convert the relative dir to an absolute path on the system.
        let sanitized_path =
            file_utils::sanitize_path(&*decoded_path, true).expect("Expected a path to directory");

        let full_path = config
            .path
            .canonicalize()
            .expect("Couldn't canonicalize path")
            .join(sanitized_path);
        info!("Requested directory size for {full_path:?}");

        let mut joinset = task_joinset.lock().await;
        joinset.spawn(async move {
            recursive_dir_size(&full_path).await.map(|dir_size| {
                info!("Finished dir size calculation for {full_path:?}");
                DirSize {
                    web_path: dir,
                    size: dir_size,
                }
            })
        });
    }
    Ok(())
}

/// This "API" is pretty shitty but frankly miniserve doesn't really need a very fancy API. Or at
/// least I hope so.
pub async fn api_command(
    command: web::Json<ApiCommand>,
    config: web::Data<MiniserveConfig>,
    task_joinset: web::Data<Mutex<DirSizeJoinSet>>,
) -> Result<impl Responder, RuntimeError> {
    match command.into_inner() {
        ApiCommand::CalculateDirSizes(dirs) => {
            handle_dir_size_tasks(dirs, &config, task_joinset).await?;
            Ok("Directories are being calculated")
        }
    }
}

pub async fn favicon() -> impl Responder {
    let logo = include_str!("../data/logo.svg");
    HttpResponse::Ok()
        .insert_header(ContentType(mime::IMAGE_SVG))
        .body(logo)
}

pub async fn css(stylesheet: web::Data<String>) -> impl Responder {
    HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_CSS))
        .body(stylesheet.to_string())
}

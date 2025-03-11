use std::{path::PathBuf, sync::Arc, time::Duration};

use actix_web::{HttpRequest, HttpResponse, Responder, http::header::ContentType, web};
use actix_web_lab::{
    sse::{self, Sse},
    util::InfallibleStream,
};
use bytesize::ByteSize;
use dav_server::{
    DavConfig, DavHandler,
    actix::{DavRequest, DavResponse},
};
use futures::future::join_all;
use log::{error, info};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, time::interval};
use tokio::{sync::mpsc, task::JoinSet};
use tokio_stream::wrappers::ReceiverStream;

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

#[derive(Debug)]
pub struct DirSizeTasks {
    tasks: Arc<Mutex<JoinSet<Result<DirSize, RuntimeError>>>>,
}

impl DirSizeTasks {
    pub fn new(show_exact_bytes: bool, sse_manager: web::Data<SseManager>) -> Self {
        let tasks = Arc::new(Mutex::new(JoinSet::<Result<DirSize, RuntimeError>>::new()));

        // Spawn a task that will periodically check for finished calculations.
        let tasks_ = tasks.clone();
        actix_web::rt::spawn(async move {
            let mut interval = interval(Duration::from_millis(50));
            loop {
                // See whether there are any calculations finished and if so dispatch a message to
                // the SSE channels.
                match tasks_.lock().await.try_join_next() {
                    Some(Ok(Ok(finished_task))) => {
                        let dir_size = if show_exact_bytes {
                            format!("{} B", finished_task.size)
                        } else {
                            ByteSize::b(finished_task.size).to_string()
                        };

                        let dir_size_reply = DirSizeReply {
                            web_path: finished_task.web_path,
                            size: dir_size,
                        };

                        let msg = sse::Data::new_json(dir_size_reply)
                            .expect("Couldn't serialize as JSON")
                            .event("dir-size");
                        sse_manager.broadcast(msg).await
                    }
                    Some(Ok(Err(e))) => {
                        error!("Some error during dir size calculation: {e}");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("Some error during dir size calculation joining: {e}");
                        break;
                    }
                    None => {
                        // If there's nothing we'll just chill a sec
                        interval.tick().await;
                    }
                };
            }
        });

        Self { tasks }
    }

    pub async fn calc_dir_size(&self, web_path: String, path: PathBuf) {
        self.tasks.lock().await.spawn(async move {
            recursive_dir_size(&path).await.map(|dir_size| {
                info!("Finished dir size calculation for {path:?}");
                DirSize {
                    web_path,
                    size: dir_size,
                }
            })
        });
    }
}

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

#[derive(Debug, Clone, Default)]
pub struct SseManager {
    clients: Arc<Mutex<Vec<mpsc::Sender<sse::Event>>>>,
}

impl SseManager {
    /// Constructs new broadcaster and spawns ping loop.
    pub fn new() -> Self {
        let clients = Arc::new(Mutex::new(Vec::<mpsc::Sender<sse::Event>>::new()));

        // Spawn a task that will periodically check for stale clients.
        let clients_ = clients.clone();
        actix_web::rt::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                // Clean up stale clients
                let clients = clients_.lock().await.clone();
                let mut ok_clients = Vec::new();
                for client in clients {
                    if client
                        .send(sse::Event::Comment("ping".into()))
                        .await
                        .is_ok()
                    {
                        // Clients that are able to receive this are still connected and the rest
                        // will be dropped.
                        ok_clients.push(client.clone());
                    } else {
                        info!("Removing a stale client");
                    }
                }
                *clients_.lock().await = ok_clients;
            }
        });

        Self { clients }
    }

    /// Registers client with broadcaster, returning an SSE response body.
    pub async fn new_client(&self) -> Sse<InfallibleStream<ReceiverStream<sse::Event>>> {
        let (tx, rx) = mpsc::channel(10);

        tx.send(sse::Data::new("Connected to SSE event stream").into())
            .await
            .unwrap();

        self.clients.lock().await.push(tx);

        Sse::from_infallible_receiver(rx)
    }

    /// Broadcasts `msg` to all clients.
    pub async fn broadcast(&self, msg: sse::Data) {
        let clients = self.clients.lock().await.clone();

        let send_futures = clients.iter().map(|client| client.send(msg.clone().into()));

        // Try to send to all clients, ignoring failures disconnected clients will get swept up by
        // `remove_stale_clients`.
        let _ = join_all(send_futures).await;
    }
}

/// SSE API route that yields an event stream that clients can subscribe to
pub async fn api_sse(sse_manager: web::Data<SseManager>) -> impl Responder {
    sse_manager.new_client().await
}

async fn handle_dir_size_tasks(
    dirs: Vec<String>,
    config: &MiniserveConfig,
    dir_size_tasks: web::Data<DirSizeTasks>,
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

        dir_size_tasks.calc_dir_size(dir, full_path).await;
    }
    Ok(())
}

/// This "API" is pretty shitty but frankly miniserve doesn't really need a very fancy API. Or at
/// least I hope so.
pub async fn api_command(
    command: web::Json<ApiCommand>,
    config: web::Data<MiniserveConfig>,
    dir_size_tasks: web::Data<DirSizeTasks>,
) -> Result<impl Responder, RuntimeError> {
    match command.into_inner() {
        ApiCommand::CalculateDirSizes(dirs) => {
            handle_dir_size_tasks(dirs, &config, dir_size_tasks).await?;
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

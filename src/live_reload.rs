use std::{convert::Infallible, fmt::Display, path::Path, time::Duration};

use actix_web::{
    Error, HttpRequest, HttpResponse, Responder,
    body::{self, EitherBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    error::ErrorInternalServerError,
    http::header::{CONTENT_TYPE, ContentType},
    middleware::Next,
    web,
};
use actix_web_lab::sse::{Data, Event, Sse};
use notify::{INotifyWatcher, RecursiveMode, Watcher as _};
use serde::Serialize;
use tokio::sync::mpsc::{self, Receiver};

#[derive(Serialize)]
enum ReloadEvent {
    Css,
    Other,
}
impl Display for ReloadEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReloadEvent::Css => write!(f, "css"),
            ReloadEvent::Other => write!(f, "other"),
        }
    }
}

/// Sends Server-Side Event when a file in [serve_path] changes.
pub async fn live_reload_handler(req: HttpRequest) -> Result<impl Responder, actix_web::Error> {
    let serve_path = req
        .app_data::<web::Data<crate::MiniserveConfig>>()
        .unwrap()
        .path
        .clone();

    let (watcher, rx) = file_change_listener(&serve_path);

    // we have to wrap the file change event receiver into a future and can't directly pass the channel
    // to actix sse because then the watcher would go out of scope and would be destroyed
    // this way, the watcher is owned by the sse stream and only dies once the connection finishes.
    let stream = futures::stream::unfold((watcher, rx), |(watcher, mut rx)| async move {
        rx.recv().await.map(|event| {
            let data = Data::new(event.to_string()).event("reload");
            (
                Ok::<Event, Infallible>(Into::<Event>::into(data)),
                (watcher, rx),
            )
        })
    });

    // send keep alive to prevent browser from closing the connection
    Ok(Sse::from_stream(stream).with_keep_alive(Duration::from_secs(5)))
}

fn file_change_listener(serve_path: &Path) -> (INotifyWatcher, Receiver<ReloadEvent>) {
    let (tx, rx) = mpsc::channel(10);

    let event_fn = move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res
            && (event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove())
        {
            let reload_event = if event.paths.iter().all(|p| {
                p.file_name()
                    .and_then(|filename| filename.to_str())
                    .is_some_and(|filename| filename.contains(".css"))
            }) {
                ReloadEvent::Css
            } else {
                ReloadEvent::Other
            };

            tx.blocking_send(reload_event).unwrap();
        }
    };

    let mut watcher = notify::recommended_watcher(event_fn).unwrap();
    watcher.watch(serve_path, RecursiveMode::Recursive).unwrap();

    (watcher, rx)
}

/// Middleware to insert JavaScript code that listens for reloads into the body
/// if it's an HTML page. The JavaScript code opens a WebSocket connection to
/// [ws_handler].
pub async fn inject_auto_reload_code_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody, String>>, Error> {
    let (req, resp) = next.call(req).await?.into_parts();

    let headers = resp.headers().clone();
    let status_code = resp.status();
    let content_type = resp.headers().get(CONTENT_TYPE);

    if let Some(content_type) = content_type.and_then(|c| c.to_str().ok())
        && content_type == ContentType::html().0
    {
        let body = body::to_bytes(resp.into_body())
            .await
            .map_err(|_| ErrorInternalServerError("failed to convert body to bytes"))?;
        let html_body = String::from_utf8(body.to_vec()).map_err(ErrorInternalServerError)?;

        let body_with_reload_js = [
            html_body.as_str(),
            "<script>",
            include_str!("../data/live_reload.js"),
            "</script>",
        ]
        .join("\n");

        let mut resp = HttpResponse::with_body(status_code, body_with_reload_js);
        *resp.headers_mut() = headers;
        Ok(ServiceResponse::new(req, resp.map_into_right_body()))
    } else {
        Ok(ServiceResponse::new(req, resp.map_into_left_body()))
    }
}

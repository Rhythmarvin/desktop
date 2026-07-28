use crate::app_state::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::header;
use axum::response::Response;
use futures_util::stream;
use std::convert::Infallible;
use tokio::sync::broadcast;

/// SSE endpoint that streams northbound events to connected web clients.
///
/// Each event is delivered as an SSE-formatted frame (`data: <json>\n\n`).
/// A comment line is sent every 15 seconds as a keep-alive to prevent
/// proxies from closing idle connections.
pub async fn northbound_events(State(state): State<AppState>) -> Response {
    let rx = state.northbound().subscribe();

    let body_stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => {
                let json = serde_json::to_string(&event).ok()?;
                let frame = format!("event: northbound\ndata: {json}\n\n");
                Some((Ok::<_, Infallible>(frame.into_bytes()), rx))
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                let frame = format!("event: gap\ndata: {{\"skipped\":{skipped}}}\n\n");
                Some((Ok(frame.into_bytes()), rx))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });

    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from_stream(body_stream))
        .unwrap()
}

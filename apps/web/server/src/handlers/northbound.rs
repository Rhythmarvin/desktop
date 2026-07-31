use crate::app_state::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::header;
use axum::response::Response;
use futures_util::stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::{Instant, MissedTickBehavior};

const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// SSE endpoint that streams northbound events to connected web clients.
///
/// Each event is delivered as an SSE-formatted frame (`data: <json>\n\n`).
/// A comment line is sent every 15 seconds as a keep-alive to prevent
/// proxies from closing idle connections.
pub async fn northbound_events(State(state): State<AppState>) -> Response {
    let rx = state.northbound().subscribe();

    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(northbound_stream(
            rx,
            KEEP_ALIVE_INTERVAL,
        )))
        .unwrap()
}

/// Interleaves northbound events with heartbeat comments so idle SSE connections stay open.
fn northbound_stream(
    rx: broadcast::Receiver<ora_contracts::Northbound>,
    keep_alive_interval: Duration,
) -> impl futures_util::Stream<Item = Result<Vec<u8>, Infallible>> {
    let mut keep_alive =
        tokio::time::interval_at(Instant::now() + keep_alive_interval, keep_alive_interval);
    keep_alive.set_missed_tick_behavior(MissedTickBehavior::Delay);

    stream::unfold((rx, keep_alive), |(mut rx, mut keep_alive)| async move {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let json = serde_json::to_string(&event).ok()?;
                        let frame = format!("event: northbound\ndata: {json}\n\n");
                        Some((Ok(frame.into_bytes()), (rx, keep_alive)))
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        let frame = format!("event: gap\ndata: {{\"skipped\":{skipped}}}\n\n");
                        Some((Ok(frame.into_bytes()), (rx, keep_alive)))
                    }
                    Err(broadcast::error::RecvError::Closed) => None,
                }
            }
            _ = keep_alive.tick() => {
                Some((Ok(b": keep-alive\n\n".to_vec()), (rx, keep_alive)))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::northbound_stream;
    use futures_util::StreamExt;
    use ora_contracts::Northbound;
    use pretty_assertions::assert_eq;
    use std::time::Duration;
    use tokio::sync::broadcast;

    /// Verifies idle streams emit an SSE comment before infrastructure can time them out.
    #[tokio::test]
    async fn emits_keep_alive_comments() {
        let (_sender, receiver) = broadcast::channel(1);
        let mut stream = Box::pin(northbound_stream(receiver, Duration::from_millis(1)));

        assert_eq!(
            stream.next().await,
            Some(Ok::<_, std::convert::Infallible>(
                b": keep-alive\n\n".to_vec()
            ))
        );
    }

    /// Verifies lag is made explicit so clients can recover from missed notifications.
    #[tokio::test]
    async fn emits_gap_after_receiver_lag() {
        let (sender, receiver) = broadcast::channel(1);
        sender
            .send(Northbound::SessionTitleUpdated {
                session_id: "session-1".to_string(),
                title: "First".to_string(),
            })
            .unwrap();
        sender
            .send(Northbound::SessionTitleUpdated {
                session_id: "session-2".to_string(),
                title: "Second".to_string(),
            })
            .unwrap();
        let mut stream = Box::pin(northbound_stream(receiver, Duration::from_secs(60)));

        assert_eq!(
            stream.next().await,
            Some(Ok::<_, std::convert::Infallible>(
                b"event: gap\ndata: {\"skipped\":1}\n\n".to_vec()
            ))
        );
    }
}

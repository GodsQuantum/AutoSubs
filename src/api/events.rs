use crate::state::AppState;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::StreamExt;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

pub async fn events(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|result| async move {
        match result {
            Ok(event) => serde_json::to_string(&event)
                .ok()
                .map(|data| Ok(Event::default().event("job").data(data))),
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    )
}

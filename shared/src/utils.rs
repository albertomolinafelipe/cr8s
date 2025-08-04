use futures_util::TryStreamExt;
use reqwest::Client;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;

/// Generic watcher for streaming API responses.
pub async fn watch_stream<T, F>(url: &str, mut handle_event: F)
where
    T: DeserializeOwned,
    F: FnMut(T) + Send + 'static,
{
    let client = Client::new();
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let byte_stream = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
            let stream_reader = StreamReader::new(byte_stream);
            let mut lines = BufReader::new(stream_reader).lines();

            tracing::debug!(url=%url, "Started watching stream");

            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<T>(&line) {
                    Ok(event) => handle_event(event),
                    Err(e) => tracing::warn!("Failed to deserialize line: {}\nError: {}", line, e),
                }
            }

            tracing::warn!(url=%url, "Watch stream ended");
        }
        Ok(resp) => tracing::error!(status=%resp.status(), "Watch request failed: HTTP"),
        Err(err) => tracing::error!(error=%err, "Watch request error"),
    }
}

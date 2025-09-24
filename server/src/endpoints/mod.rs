pub mod log;
mod nodes;
mod pods;
mod replicasets;

use actix_web::web::{self, scope};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(scope("/nodes").configure(nodes::config))
        .service(scope("/pods").configure(pods::config))
        .service(scope("/replicasets").configure(replicasets::config));
}

#[cfg(test)]
pub mod helpers {
    use actix_web::{body::MessageBody, dev::ServiceResponse};
    use serde::de::DeserializeOwned;

    pub async fn collect_stream_events<T, B>(
        resp: ServiceResponse<B>,
        events: &mut Vec<T>,
        limit: usize,
    ) where
        T: DeserializeOwned + Clone,
        B: MessageBody + Unpin,
    {
        let mut body = Box::pin(resp.into_body());

        while let Some(chunk_result) =
            futures::future::poll_fn(|cx| body.as_mut().poll_next(cx)).await
        {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(_) => panic!("stream error"),
            };

            let text = std::str::from_utf8(&chunk).expect("invalid utf8");

            for line in text.lines() {
                if line.is_empty() {
                    continue;
                }

                let event: T = serde_json::from_str(line)
                    .expect("failed to deserialize event from stream line");

                events.push(event);
                if events.len() >= limit {
                    return;
                }
            }
        }
    }
}

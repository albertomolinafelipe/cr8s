use actix_web::Error;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use futures_util::future::{LocalBoxFuture, Ready, ok};
use std::task::{Context, Poll};
use tracing::{error, trace, warn};

pub struct Logging;

impl<S, B> Transform<S, ServiceRequest> for Logging
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = LoggingMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(LoggingMiddleware { service })
    }
}

pub struct LoggingMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for LoggingMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let method = req.method().clone();
        let path = req.path().to_owned();

        // trace!("{method} {path}");

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            let status = res.status();
            let code = status.as_u16();
            let log = format!("{code} {method} {path}");

            match status {
                s if s.is_success() => trace!("{log}"),
                s if s.is_client_error() => warn!("{log}"),
                s if s.is_server_error() => error!("{log}"),
                _ => trace!("{log}"),
            }

            Ok(res)
        })
    }
}

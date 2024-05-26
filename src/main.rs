#![deny(rust_2018_idioms)]
#![warn(missing_debug_implementations)]

use std::{
    env,
    future::Future,
    net::{Ipv4Addr, UdpSocket},
    pin::Pin,
    time::Duration,
};

use actix_http::{HttpService, Request};
use actix_server::Server;
use cadence::{QueuingMetricSink, UdpMetricSink};
use reqwest::redirect::Policy as RedirectPolicy;
use tracing::Instrument as _;

mod engine;
mod interactors;
mod models;
mod parsers;
mod server;
mod utils;

use self::{engine::Engine, server::App, utils::index::ManagedIndex};

/// Future crate's BoxFuture without the explicit lifetime parameter.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

const DEPS_RS_UA: &str = "deps.rs";

fn init_metrics() -> QueuingMetricSink {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let host = ("127.0.0.1", 8125);
    let sink = UdpMetricSink::from(host, socket).unwrap();
    QueuingMetricSink::from(sink)
}

fn init_tracing_subscriber() {
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let stdout_logger = match env::var("RUST_LOG_TIME").as_deref() {
        Ok("false") => fmt::layer().without_time().boxed(),
        _ => fmt::layer().boxed(),
    };

    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(stdout_logger)
        .init();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenvy::dotenv().ok();
    init_tracing_subscriber();
    let metrics = init_metrics();

    let client = reqwest::Client::builder()
        .user_agent(DEPS_RS_UA)
        .redirect(RedirectPolicy::limited(5))
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("could not read port");

    let index = ManagedIndex::new();

    {
        let index = index.clone();

        tokio::spawn(async move {
            index.refresh_at_interval(Duration::from_secs(20)).await;
        });
    }

    let mut engine = Engine::new(client.clone(), index);
    engine.set_metrics(metrics);

    let server = Server::build()
        .bind("deps-rs", (Ipv4Addr::UNSPECIFIED, port), move || {
            let engine = engine.clone();

            let app = App::new(engine.clone());

            HttpService::build()
                .client_disconnect_timeout(Duration::from_secs(5))
                .client_request_timeout(Duration::from_secs(5))
                .finish(move |req: Request| {
                    let app = app.clone();
                    let path = req.path().to_owned();

                    async move {
                        app.handle(req)
                            .instrument(tracing::info_span!("@", %path))
                            .await
                    }
                })
                .tcp_auto_h2c()
        })
        .unwrap()
        .run();

    tracing::info!("Server running on port {port}");

    if let Err(err) = server.await {
        tracing::error!("server error: {err}");
    }
}

#![deny(rust_2018_idioms)]
#![warn(missing_debug_implementations)]

use std::{
    env,
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    pin::Pin,
    time::Duration,
};

use cadence::{QueuingMetricSink, UdpMetricSink};
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Server,
};

use reqwest::redirect::Policy as RedirectPolicy;
use slog::{error, info, o, Drain, Logger};

mod engine;
mod interactors;
mod models;
mod parsers;
mod server;
mod utils;

use self::engine::Engine;
use self::server::App;
use self::utils::index::ManagedIndex;

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

fn init_root_logger() -> Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    Logger::root(drain, o!())
}

#[tokio::main]
async fn main() {
    let logger = init_root_logger();

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

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    let mut managed_index = ManagedIndex::new(Duration::from_secs(20), logger.clone());
    let _ = managed_index.initial_clone().await;

    let index = managed_index.index();
    tokio::spawn(async move {
        managed_index.refresh_at_interval().await;
    });

    let mut engine = Engine::new(client.clone(), index, logger.new(o!()));
    engine.set_metrics(metrics);

    let svc_logger = logger.new(o!());
    let make_svc = make_service_fn(move |_socket: &AddrStream| {
        let engine = engine.clone();
        let logger = svc_logger.clone();

        async move {
            let server = App::new(logger.clone(), engine.clone());
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let server = server.clone();
                async move { server.handle(req).await }
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_svc);

    info!(logger, "Server running on port {}", port);

    if let Err(e) = server.await {
        error!(logger, "server error: {}", e);
    }
}

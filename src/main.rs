#![feature(proc_macro_hygiene)]

extern crate badge;
extern crate cadence;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate indexmap;
#[macro_use]
extern crate lazy_static;
extern crate lru_cache;
extern crate maud;
extern crate relative_path;
extern crate route_recognizer;
extern crate rustsec;
extern crate semver;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate shared_failure;
#[macro_use]
extern crate slog;
extern crate slog_json;
extern crate tokio_service;
extern crate tokio_threadpool;
extern crate toml;
#[macro_use]
extern crate try_future;

mod engine;
mod interactors;
mod models;
mod parsers;
mod server;
mod utils;

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Mutex;

use cadence::{QueuingMetricSink, UdpMetricSink};
use futures::Future;
use hyper::Client;
use hyper_tls::HttpsConnector;
use slog::Drain;

use self::engine::Engine;
use self::server::Server;

fn init_metrics() -> QueuingMetricSink {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let host = ("127.0.0.1", 8125);
    let sink = UdpMetricSink::from(host, socket).unwrap();
    QueuingMetricSink::from(sink)
}

fn main() {
    let logger = slog::Logger::root(
        Mutex::new(slog_json::Json::default(std::io::stderr())).map(slog::Fuse),
        o!("version" => env!("CARGO_PKG_VERSION")),
    );

    let metrics = init_metrics();

    let connector = HttpsConnector::new(4).expect("failed to create https connector");
    let client = Client::builder().build::<_, hyper::Body>(connector);

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("could not read port");

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    let mut engine = Engine::new(client.clone(), logger.clone());
    engine.set_metrics(metrics);

    let server = Server::new(logger.clone(), engine);
    let hyper_server = hyper::server::Server::bind(&addr)
        .serve(move || futures::future::ok::<_, &'static str>(server.clone()))
        .map_err(move |err| info!(logger.clone(), "server connection error: {}", err));
    println!("Server running on port {}", port);
    hyper::rt::run(hyper_server);
}

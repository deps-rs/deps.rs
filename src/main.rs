#![deny(rust_2018_idioms)]
#![allow(unused)]

#[macro_use]
extern crate try_future;

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Mutex;

use cadence::{QueuingMetricSink, UdpMetricSink};
use futures::{Future, Stream};
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn_ok};
use hyper::Client;
use hyper_tls::HttpsConnector;
use slog::Drain;
use slog::{info, o};
use tokio_core::reactor::Core;

mod engine;
mod interactors;
mod models;
mod parsers;
mod server;
mod utils;

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

    let mut core = Core::new().expect("failed to create event loop");

    let handle = core.handle();

    let connector = HttpsConnector::new(4).expect("failed to create https connector");

    let client = Client::builder().build(connector);

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("could not read port");

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    let mut engine = Engine::new(client.clone(), logger.clone());
    engine.set_metrics(metrics);

    let server = Server::new(logger.clone(), engine);
    let make_svc = make_service_fn(move |socket: &AddrStream| {
        let server = server.clone();
        futures::future::ok::<_, hyper::Error>(server)
    });
    let server = hyper::Server::bind(&addr).serve(make_svc);

    println!("Server running on port {}", port);
    hyper::rt::run(server.map_err(|e| {
        eprintln!("server error: {}", e);
    }));
}

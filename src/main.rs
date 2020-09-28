#![deny(rust_2018_idioms)]
#![allow(unused)]


#[macro_use]
extern crate try_future;

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Mutex;

use cadence::{QueuingMetricSink, UdpMetricSink};
use futures::{Future, Stream};
use hyper::server::Http;
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

    let connector = HttpsConnector::new(4, &handle).expect("failed to create https connector");

    let client = Client::configure()
        .connector(connector)
        .build(&core.handle());

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("could not read port");

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    let http = Http::new();

    let mut engine = Engine::new(client.clone(), logger.clone());
    engine.set_metrics(metrics);

    let server = Server::new(logger.clone(), engine);

    let serve = http
        .serve_addr_handle(&addr, &handle, move || Ok(server.clone()))
        .expect("failed to bind server");

    let serving = serve.for_each(move |conn| {
        let conn_logger = logger.clone();
        handle.spawn(conn.then(move |res| {
            if let Err(err) = res {
                info!(conn_logger, "server connection error: {}", err)
            }
            Ok(())
        }));
        Ok(())
    });

    println!("Server running on port {}", port);

    core.run(serving).expect("server failed");
}

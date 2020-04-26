#![feature(conservative_impl_trait)]
#![feature(ip_constructors)]
#![feature(proc_macro)]
#![feature(proc_macro_hygiene)]

extern crate badge;
extern crate cadence;
#[macro_use] extern crate failure;
#[macro_use] extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate indexmap;
#[macro_use] extern crate lazy_static;
extern crate lru_cache;
extern crate maud;
extern crate relative_path;
extern crate route_recognizer;
extern crate rustsec;
extern crate semver;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate shared_failure;
#[macro_use] extern crate slog;
extern crate slog_json;
extern crate tokio_core;
extern crate tokio_service;
extern crate toml;
#[macro_use] extern crate try_future;

mod utils;
mod models;
mod parsers;
mod interactors;
mod engine;
mod server;

use std::env;
use std::net::{IpAddr, Ipv4Addr, UdpSocket, SocketAddr};
use std::sync::Mutex;

use cadence::{QueuingMetricSink, UdpMetricSink};
use futures::{Future, Stream};
use hyper::Client;
use hyper::server::Http;
use hyper_tls::HttpsConnector;
use slog::Drain;
use tokio_core::reactor::Core;

use self::server::Server;
use self::engine::Engine;

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
        o!("version" => env!("CARGO_PKG_VERSION"))
    );

    let metrics = init_metrics();

    let mut core = Core::new()
        .expect("failed to create event loop");

    let handle = core.handle();

    let connector = HttpsConnector::new(4, &handle)
        .expect("failed to create https connector");

    let client = Client::configure()
        .connector(connector)
        .build(&core.handle());

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string()).parse()
        .expect("could not read port");

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    let http = Http::new();

    let mut engine = Engine::new(client.clone(), logger.clone());
    engine.set_metrics(metrics);

    let server = Server::new(logger.clone(), engine);

    let serve = http.serve_addr_handle(&addr, &handle, move || Ok(server.clone()))
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

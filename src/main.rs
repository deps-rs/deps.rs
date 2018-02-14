#![feature(ascii_ctype)]
#![feature(conservative_impl_trait)]
#![feature(ip_constructors)]
#![feature(proc_macro)]

extern crate badge;
#[macro_use] extern crate failure;
#[macro_use] extern crate futures;
extern crate hyper;
extern crate hyper_tls;
#[macro_use] extern crate lazy_static;
extern crate lru_cache;
extern crate maud;
extern crate ordermap;
extern crate relative_path;
extern crate route_recognizer;
extern crate semver;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate serde_json;
#[macro_use] extern crate slog;
extern crate slog_json;
extern crate tokio_core;
extern crate tokio_service;
extern crate toml;

mod utils;
mod models;
mod parsers;
mod interactors;
mod engine;
mod server;

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;

use futures::{Future, Stream};
use hyper::Client;
use hyper::server::Http;
use hyper_tls::HttpsConnector;
use slog::Drain;
use tokio_core::reactor::Core;

use self::server::Server;
use self::engine::Engine;

fn main() {
    let logger = slog::Logger::root(
        Mutex::new(slog_json::Json::default(std::io::stderr())).map(slog::Fuse),
        o!("version" => env!("CARGO_PKG_VERSION"))
    );

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

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::unspecified()), port);

    let http = Http::new();

    let engine = Engine::new(client.clone(), logger.clone());

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

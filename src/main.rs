#![feature(ascii_ctype)]
#![feature(conservative_impl_trait)]

extern crate futures;
extern crate hyper;
extern crate hyper_tls;
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

mod models;
mod parsers;
mod interactors;
mod engine;
mod assets;
mod api;

use std::net::SocketAddr;
use std::sync::Mutex;

use futures::{Future, Stream};
use hyper::Client;
use hyper::server::Http;
use hyper_tls::HttpsConnector;
use slog::Drain;
use tokio_core::reactor::Core;

use self::api::Api;
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

    let addr = "0.0.0.0:8080".parse::<SocketAddr>()
        .expect("failed to parse socket addr");

    let http = Http::new();

    let engine = Engine {
        client: client.clone(),
        logger: logger.clone()
    };

    let api = Api::new(engine);

    let serve = http.serve_addr_handle(&addr, &handle, move || Ok(api.clone()))
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

    core.run(serving).expect("server failed");
}

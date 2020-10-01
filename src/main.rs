#![deny(rust_2018_idioms)]
#![warn(missing_debug_implementations)]

use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Mutex,
};

use cadence::{QueuingMetricSink, UdpMetricSink};
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Client, Server,
};
use hyper_tls::HttpsConnector;
use slog::{o, Drain};

mod engine;
mod interactors;
mod models;
mod parsers;
mod server;
mod utils;

use self::engine::Engine;
use self::server::App;

fn init_metrics() -> QueuingMetricSink {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let host = ("127.0.0.1", 8125);
    let sink = UdpMetricSink::from(host, socket).unwrap();
    QueuingMetricSink::from(sink)
}

#[tokio::main]
async fn main() {
    let logger = slog::Logger::root(
        Mutex::new(slog_json::Json::default(std::io::stderr())).map(slog::Fuse),
        o!("version" => env!("CARGO_PKG_VERSION")),
    );

    let metrics = init_metrics();
    let client = Client::builder().build(HttpsConnector::new());

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("could not read port");

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    let mut engine = Engine::new(client.clone(), logger.clone());
    engine.set_metrics(metrics);

    let make_svc = make_service_fn(move |_socket: &AddrStream| {
        let logger = logger.clone();
        let engine = engine.clone();

        async move {
            let server = App::new(logger.clone(), engine.clone());
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let server = server.clone();
                async move { server.handle(req).await }
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_svc);

    println!("Server running on port {}", port);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

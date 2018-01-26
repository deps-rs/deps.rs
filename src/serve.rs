use futures::{Future, future};
use hyper::{Client, Error as HyperError, Request, Response, StatusCode};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use slog::Logger;
use tokio_service::Service;

use ::robots::crates::query_crates_versions;

pub struct Serve {
    pub client: Client<HttpsConnector<HttpConnector>>,
    pub logger: Logger
}

impl Service for Serve {
    type Request = Request;
    type Response = Response;
    type Error = HyperError;
    type Future = Box<Future<Item=Response, Error=HyperError>>;

    fn call(&self, req: Request) -> Self::Future {
        let crate_name = "hyper".parse().unwrap();

        let future = query_crates_versions(self.client.clone(), &crate_name).then(|result| {
            match result {
                Err(err) => {
                    let mut response = Response::new();
                    response.set_status(StatusCode::InternalServerError);
                    response.set_body(format!("{:?}", err));
                    future::Either::A(future::ok(response))
                },
                Ok(crates_response) => {
                    let mut response = Response::new();
                    response.set_body(format!("{:?}", crates_response));
                    future::Either::B(future::ok(response))
                }
            }
        });

        Box::new(future)
    }
}

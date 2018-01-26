use futures::{Future, Stream, IntoFuture, future};
use hyper::{Error as HyperError, Method, Request, Response, StatusCode};
use hyper::error::UriError;
use tokio_service::Service;
use serde_json;

use ::models::crates::CrateName;

const CRATES_API_BASE_URI: &'static str = "https://crates.io/api/v1";

#[derive(Serialize, Deserialize, Debug)]
pub struct CratesVersion {
    num: String,
    yanked: bool
}

#[derive(Serialize, Deserialize, Debug)]
pub struct QueryCratesVersionsResponse {
    versions: Vec<CratesVersion>
}

#[derive(Debug)]
pub enum QueryCratesVersionsError {
    Uri(UriError),
    Status(StatusCode),
    Transport(HyperError),
    Decode(serde_json::Error)
}

pub fn query_crates_versions<S>(service: S, crate_name: &CrateName) ->
    impl Future<Item=QueryCratesVersionsResponse, Error=QueryCratesVersionsError>
    where S: Service<Request=Request, Response=Response, Error=HyperError>
{
    let uri_future = format!("{}/crates/{}/versions", CRATES_API_BASE_URI, crate_name.as_ref())
        .parse().into_future().map_err(QueryCratesVersionsError::Uri);

    uri_future.and_then(move |uri| {
        let request = Request::new(Method::Get, uri);

        service.call(request).map_err(QueryCratesVersionsError::Transport).and_then(|response| {
            let status = response.status();
            if !status.is_success() {
                future::Either::A(future::err(QueryCratesVersionsError::Status(status)))
            } else {
                let body_future = response.body().concat2().map_err(QueryCratesVersionsError::Transport);
                let decode_future = body_future
                    .and_then(|body| serde_json::from_slice(&body).map_err(QueryCratesVersionsError::Decode));
                future::Either::B(decode_future)
            }
        })
    })
}

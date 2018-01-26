use futures::{Future, Stream, IntoFuture, future};
use hyper::{Error as HyperError, Method, Request, Response, StatusCode};
use hyper::error::UriError;
use tokio_service::Service;
use semver::Version;
use serde_json;

use ::models::crates::{CrateName, CrateRelease};

const CRATES_API_BASE_URI: &'static str = "https://crates.io/api/v1";

#[derive(Serialize, Deserialize, Debug)]
struct CratesVersion {
    num: Version,
    yanked: bool
}

#[derive(Serialize, Deserialize, Debug)]
struct QueryCratesVersionsBody {
    versions: Vec<CratesVersion>
}

fn convert_body(name: &CrateName, body: QueryCratesVersionsBody) -> Result<QueryCrateResponse, QueryCrateError> {
    let releases = body.versions.into_iter().map(|version| {
        CrateRelease {
            name: name.clone(),
            version: version.num,
            yanked: version.yanked
        }
    }).collect();

    Ok(QueryCrateResponse {
        releases: releases
    })
}

pub struct QueryCrateResponse {
    pub releases: Vec<CrateRelease>
}

#[derive(Debug)]
pub enum QueryCrateError {
    Uri(UriError),
    Status(StatusCode),
    Transport(HyperError),
    Decode(serde_json::Error)
}

pub fn query_crate<S>(service: S, crate_name: CrateName) ->
    impl Future<Item=QueryCrateResponse, Error=QueryCrateError>
    where S: Service<Request=Request, Response=Response, Error=HyperError>
{
    let uri_future = format!("{}/crates/{}/versions", CRATES_API_BASE_URI, crate_name.as_ref())
        .parse().into_future().map_err(QueryCrateError::Uri);

    uri_future.and_then(move |uri| {
        let request = Request::new(Method::Get, uri);

        service.call(request).map_err(QueryCrateError::Transport).and_then(move |response| {
            let status = response.status();
            if !status.is_success() {
                future::Either::A(future::err(QueryCrateError::Status(status)))
            } else {
                let body_future = response.body().concat2().map_err(QueryCrateError::Transport);
                let decode_future = body_future.and_then(|body| {
                        serde_json::from_slice::<QueryCratesVersionsBody>(&body)
                            .map_err(QueryCrateError::Decode)
                    });
                let convert_future = decode_future.and_then(move |body| convert_body(&crate_name, body));
                future::Either::B(convert_future)
            }
        })
    })
}

use failure::Error;
use futures::{Future, Stream, IntoFuture, future};
use hyper::{Error as HyperError, Method, Request, Response, Uri};
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

fn convert_body(name: &CrateName, body: QueryCratesVersionsBody) -> Result<QueryCrateResponse, Error> {
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

pub fn query_crate<S>(service: S, crate_name: CrateName) ->
    impl Future<Item=QueryCrateResponse, Error=Error>
    where S: Service<Request=Request, Response=Response, Error=HyperError>
{
    let uri_future = format!("{}/crates/{}/versions", CRATES_API_BASE_URI, crate_name.as_ref())
        .parse::<Uri>().into_future().from_err();

    uri_future.and_then(move |uri| {
        let request = Request::new(Method::Get, uri.clone());

        service.call(request).from_err().and_then(move |response| {
            let status = response.status();
            if !status.is_success() {
                future::Either::A(future::err(format_err!("Status code {} for URI {}", status, uri)))
            } else {
                let body_future = response.body().concat2().from_err();
                let decode_future = body_future.and_then(|body| {
                        serde_json::from_slice::<QueryCratesVersionsBody>(&body)
                            .map_err(|err| err.into())
                    });
                let convert_future = decode_future.and_then(move |body| convert_body(&crate_name, body));
                future::Either::B(convert_future)
            }
        })
    })
}

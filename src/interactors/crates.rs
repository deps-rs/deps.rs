use std::str;

use failure::Error;
use futures::{Future, Stream, IntoFuture, future};
use hyper::{Error as HyperError, Method, Request, Response, Uri};
use tokio_service::Service;
use semver::Version;
use serde_json;

use ::models::crates::{CrateName, CrateRelease};

const CRATES_INDEX_BASE_URI: &str = "https://raw.githubusercontent.com/rust-lang/crates.io-index";

#[derive(Deserialize, Debug)]
struct RegistryPackage {
    vers: Version,
    #[serde(default)]
    yanked: bool
}

fn convert_pkgs(name: &CrateName, packages: Vec<RegistryPackage>) -> Result<QueryCrateResponse, Error> {
    let releases = packages.into_iter().map(|package| {
        CrateRelease {
            name: name.clone(),
            version: package.vers,
            yanked: package.yanked
        }
    }).collect();

    Ok(QueryCrateResponse {
        releases: releases
    })
}

pub struct QueryCrateResponse {
    pub releases: Vec<CrateRelease>
}

#[derive(Debug, Clone)]
pub struct QueryCrate<S>(pub S);

impl<S> Service for QueryCrate<S>
    where S: Service<Request=Request, Response=Response, Error=HyperError> + Clone + 'static,
          S::Future: 'static
{
    type Request = CrateName;
    type Response = QueryCrateResponse;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self,  crate_name: CrateName) -> Self::Future {
        let service = self.0.clone();

        let lower_name = crate_name.as_ref().to_lowercase();

        let path = match lower_name.len() {
            1 => format!("1/{}", lower_name),
            2 => format!("2/{}", lower_name),
            3 => format!("3/{}/{}", &lower_name[..1], lower_name),
            _ => format!("{}/{}/{}", &lower_name[0..2], &lower_name[2..4], lower_name),
        };

        let uri_future = format!("{}/master/{}", CRATES_INDEX_BASE_URI, path)
            .parse::<Uri>().into_future().from_err();

        Box::new(uri_future.and_then(move |uri| {
            let request = Request::new(Method::Get, uri.clone());

            service.call(request).from_err().and_then(move |response| {
                let status = response.status();
                if !status.is_success() {
                    future::Either::A(future::err(format_err!("Status code {} for URI {}", status, uri)))
                } else {
                    let body_future = response.body().concat2().from_err();
                    let decode_future = body_future.and_then(|body| {
                        let string_body = str::from_utf8(body.as_ref())?;
                        let packages = string_body.lines()
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .map(|s| serde_json::from_str::<RegistryPackage>(s))
                            .collect::<Result<_, _>>()?;
                        Ok(packages)
                    });
                    let convert_future = decode_future.and_then(move |pkgs| convert_pkgs(&crate_name, pkgs));
                    future::Either::B(convert_future)
                }
            })
        }))
    }
}

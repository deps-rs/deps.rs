use std::str;

use anyhow::{anyhow, ensure, Error};
use futures::{future, Future, IntoFuture, Stream};
use hyper::{Body, Error as HyperError, Method, Request, Response, Uri, header::USER_AGENT};
use semver::{Version, VersionReq};
use serde::Deserialize;
use tokio_service::Service;

use crate::models::crates::{CrateDep, CrateDeps, CrateName, CratePath, CrateRelease};

const CRATES_INDEX_BASE_URI: &str = "https://raw.githubusercontent.com/rust-lang/crates.io-index";
const CRATES_API_BASE_URI: &str = "https://crates.io/api/v1";

#[derive(Deserialize, Debug)]
struct RegistryPackageDep {
    name: String,
    req: VersionReq,
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Deserialize, Debug)]
struct RegistryPackage {
    vers: Version,
    #[serde(default)]
    deps: Vec<RegistryPackageDep>,
    #[serde(default)]
    yanked: bool,
}

fn convert_pkgs(
    name: &CrateName,
    packages: Vec<RegistryPackage>,
) -> Result<QueryCrateResponse, Error> {
    let releases = packages
        .into_iter()
        .map(|package| {
            let mut deps = CrateDeps::default();
            for dep in package.deps {
                match dep
                    .kind
                    .map(|k| k.clone())
                    .unwrap_or_else(|| "normal".into())
                    .as_ref()
                {
                    "normal" => deps
                        .main
                        .insert(dep.name.parse()?, CrateDep::External(dep.req)),
                    "dev" => deps
                        .dev
                        .insert(dep.name.parse()?, CrateDep::External(dep.req)),
                    _ => None,
                };
            }
            Ok(CrateRelease {
                name: name.clone(),
                version: package.vers,
                deps: deps,
                yanked: package.yanked,
            })
        })
        .collect::<Result<_, Error>>()?;

    Ok(QueryCrateResponse { releases: releases })
}

pub struct QueryCrateResponse {
    pub releases: Vec<CrateRelease>,
}

#[derive(Debug, Clone)]
pub struct QueryCrate<S>(pub S);

impl<S> Service for QueryCrate<S>
where
    S: Service<Request = Request<Body>, Response = Response<Body>, Error = HyperError>
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    type Request = CrateName;
    type Response = QueryCrateResponse;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error> + Send>;

    fn call(&self, crate_name: CrateName) -> Self::Future {
        let lower_name = crate_name.as_ref().to_lowercase();

        let path = match lower_name.len() {
            1 => format!("1/{}", lower_name),
            2 => format!("2/{}", lower_name),
            3 => format!("3/{}/{}", &lower_name[..1], lower_name),
            _ => format!("{}/{}/{}", &lower_name[0..2], &lower_name[2..4], lower_name),
        };

        let uri =
            try_future_box!(format!("{}/master/{}", CRATES_INDEX_BASE_URI, path).parse::<Uri>());

        let request = Request::get(uri.clone())
            .header(USER_AGENT, "deps.rs")
            .body(Body::empty()).unwrap();

        Box::new(self.0.call(request).from_err().and_then(move |response| {
            let status = response.status();
            if !status.is_success() {
                try_future!(Err(anyhow!("Status code {} for URI {}", status, uri)));
            }

            let body_future = response.into_body().concat2().from_err();
            let decode_future = body_future.and_then(move |body| {
                let string_body = str::from_utf8(body.as_ref())?;
                let packages = string_body
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| serde_json::from_str::<RegistryPackage>(s))
                    .collect::<Result<_, _>>()?;
                Ok(packages)
            });

            decode_future
                .and_then(move |pkgs| convert_pkgs(&crate_name, pkgs))
                .into()
        }))
    }
}

#[derive(Deserialize)]
struct SummaryResponseDetail {
    name: String,
    max_version: Version,
}

#[derive(Deserialize)]
struct SummaryResponse {
    most_downloaded: Vec<SummaryResponseDetail>,
}

fn convert_summary(response: SummaryResponse) -> Result<Vec<CratePath>, Error> {
    response
        .most_downloaded
        .into_iter()
        .map(|detail| {
            let name = detail.name.parse()?;
            Ok(CratePath {
                name,
                version: detail.max_version,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct GetPopularCrates<S>(pub S);

impl<S> Service for GetPopularCrates<S>
where
    S: Service<Request = Request<Body>, Response = Response<Body>, Error = HyperError>
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    type Request = ();
    type Response = Vec<CratePath>;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error> + Send>;

    fn call(&self, _req: ()) -> Self::Future {
        let service = self.0.clone();

        let uri = format!("{}/summary", CRATES_API_BASE_URI)
            .parse::<Uri>()
            .unwrap();
        let request = Request::get(uri.clone())
            .header(USER_AGENT, "deps.rs")
            .body(Body::empty()).unwrap();

        Box::new(service.call(request).from_err().and_then(move |response| {
            let status = response.status();
            if !status.is_success() {
                future::Either::A(future::err(anyhow!(
                    "Status code {} for URI {}",
                    status,
                    uri
                )))
            } else {
                let body_future = response.into_body().concat2().from_err();
                let decode_future = body_future.and_then(|body| {
                    let summary = serde_json::from_slice::<SummaryResponse>(&body)?;
                    convert_summary(summary)
                });
                future::Either::B(decode_future)
            }
        }))
    }
}

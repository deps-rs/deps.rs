use std::pin::Pin;
use std::{future::Future, task::Poll};
use std::{str, task::Context};

use anyhow::{anyhow, Error};
use futures::{
    future::{err, ok, ready},
    TryFutureExt,
};
use hyper::{
    body, header::USER_AGENT, service::Service, Body, Error as HyperError, Request, Response, Uri,
};
use semver::{Version, VersionReq};
use serde::Deserialize;

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

#[derive(Debug, Clone)]
pub struct QueryCrateResponse {
    pub releases: Vec<CrateRelease>,
}

#[derive(Debug, Clone)]
pub struct QueryCrate<S>(pub S);

impl<S> Service<CrateName> for QueryCrate<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = HyperError> + Clone,
    S::Future: Send + 'static,
{
    type Response = QueryCrateResponse;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx).map_err(|err| err.into())
    }

    fn call(&mut self, crate_name: CrateName) -> Self::Future {
        let lower_name = crate_name.as_ref().to_lowercase();

        let path = match lower_name.len() {
            1 => format!("1/{}", lower_name),
            2 => format!("2/{}", lower_name),
            3 => format!("3/{}/{}", &lower_name[..1], lower_name),
            _ => format!("{}/{}/{}", &lower_name[0..2], &lower_name[2..4], lower_name),
        };

        let uri = format!("{}/master/{}", CRATES_INDEX_BASE_URI, path);

        println!("analyze from uri {:?}", &uri);

        let uri = uri.parse::<Uri>().expect("TODO: MAP ERROR PROPERLY");

        let request = Request::get(uri.clone())
            .header(USER_AGENT, "deps.rs")
            .body(Body::empty())
            .unwrap();

        Box::pin(
            self.0
                .call(request)
                .map_err(|err| err.into())
                .and_then(move |response| {
                    let status = response.status();
                    if !status.is_success() {
                        return err(anyhow!("Status code {} for URI {}", status, uri));
                    }

                    ok(response)
                })
                .and_then(|response| body::to_bytes(response.into_body()).err_into())
                .and_then(|body| ready(String::from_utf8(body.to_vec())).err_into())
                .and_then(|string_body| {
                    ready(
                        string_body
                            .lines()
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .map(|s| serde_json::from_str::<RegistryPackage>(s))
                            .collect::<Result<_, _>>(),
                    )
                    .err_into()
                })
                .and_then(move |pkgs| ready(convert_pkgs(&crate_name, pkgs))),
        )
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

impl<S> Service<()> for GetPopularCrates<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = HyperError> + Clone,
    S::Future: Send + 'static,
{
    type Response = Vec<CratePath>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx).map_err(|err| err.into())
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let mut service = self.0.clone();

        let uri = format!("{}/summary", CRATES_API_BASE_URI);
        let uri = uri.parse::<Uri>().unwrap();
        let request = Request::get(uri.clone())
            .header(USER_AGENT, "deps.rs")
            .body(Body::empty())
            .unwrap();

        Box::pin(
            service
                .call(request)
                .map_err(|err| err.into())
                .and_then(move |response| {
                    let status = response.status();
                    if !status.is_success() {
                        err(anyhow!("Status code {} for URI {}", status, uri))
                    } else {
                        ok(response)
                    }
                })
                .and_then(|response| body::to_bytes(response.into_body()).err_into())
                .and_then(|bytes| {
                    ready(serde_json::from_slice::<SummaryResponse>(&bytes)).err_into()
                })
                .and_then(|summary| ready(convert_summary(summary)).err_into()),
        )
    }
}

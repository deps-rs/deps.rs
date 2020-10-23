use std::{fmt, str, task::Context, task::Poll};

use anyhow::Error;
use futures::FutureExt as _;
use hyper::service::Service;
use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::{
    models::crates::{CrateDep, CrateDeps, CrateName, CratePath, CrateRelease},
    BoxFuture,
};

const CRATES_INDEX_BASE_URI: &str = "https://raw.githubusercontent.com/rust-lang/crates.io-index";
const CRATES_API_BASE_URI: &str = "https://crates.io/api/v1";

#[derive(Deserialize, Debug)]
struct RegistryPackageDep {
    name: String,
    req: VersionReq,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    package: Option<String>,
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
                let name = dep.package.as_deref().unwrap_or(&dep.name).parse()?;
                match dep.kind.as_deref().unwrap_or("normal") {
                    "normal" => deps.main.insert(name, CrateDep::External(dep.req)),
                    "dev" => deps.dev.insert(name, CrateDep::External(dep.req)),
                    _ => None,
                };
            }
            Ok(CrateRelease {
                name: name.clone(),
                version: package.vers,
                deps,
                yanked: package.yanked,
            })
        })
        .collect::<Result<_, Error>>()?;

    Ok(QueryCrateResponse { releases })
}

#[derive(Debug, Clone)]
pub struct QueryCrateResponse {
    pub releases: Vec<CrateRelease>,
}

#[derive(Clone)]
pub struct QueryCrate {
    client: reqwest::Client,
}

impl QueryCrate {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn query(
        client: reqwest::Client,
        crate_name: CrateName,
    ) -> anyhow::Result<QueryCrateResponse> {
        let lower_name = crate_name.as_ref().to_lowercase();

        let path = match lower_name.len() {
            1 => format!("1/{}", lower_name),
            2 => format!("2/{}", lower_name),
            3 => format!("3/{}/{}", &lower_name[..1], lower_name),
            _ => format!("{}/{}/{}", &lower_name[0..2], &lower_name[2..4], lower_name),
        };

        let url = format!("{}/HEAD/{}", CRATES_INDEX_BASE_URI, path);
        let res = client.get(&url).send().await?.error_for_status()?;

        let string_body = res.text().await?;

        let pkgs = string_body
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;

        convert_pkgs(&crate_name, pkgs)
    }
}

impl fmt::Debug for QueryCrate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QueryCrate")
    }
}

impl Service<CrateName> for QueryCrate {
    type Response = QueryCrateResponse;
    type Error = Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, crate_name: CrateName) -> Self::Future {
        let client = self.client.clone();
        Self::query(client, crate_name).boxed()
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

#[derive(Clone, Default)]
pub struct GetPopularCrates {
    client: reqwest::Client,
}

impl GetPopularCrates {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn query(client: reqwest::Client) -> anyhow::Result<Vec<CratePath>> {
        let url = format!("{}/summary", CRATES_API_BASE_URI);
        let res = client.get(&url).send().await?.error_for_status()?;

        let summary: SummaryResponse = res.json().await?;
        convert_summary(summary)
    }
}

impl fmt::Debug for GetPopularCrates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GetPopularCrates")
    }
}
impl Service<()> for GetPopularCrates {
    type Response = Vec<CratePath>;
    type Error = Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let client = self.client.clone();
        Self::query(client).boxed()
    }
}

use std::{fmt, str, task::Context, task::Poll};

use anyhow::{anyhow, Error};
use crates_index::{Crate, DependencyKind, Index};
use futures::FutureExt as _;
use hyper::service::Service;
use semver::{Version, VersionReq};
use serde::Deserialize;
use tokio::task::spawn_blocking;

use crate::{
    models::crates::{CrateDep, CrateDeps, CrateName, CratePath, CrateRelease},
    BoxFuture,
};

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

fn convert_pkgs(krate: Crate) -> Result<QueryCrateResponse, Error> {
    let name: CrateName = krate.name().parse()?;

    let releases = krate
        .versions()
        .iter()
        .map(|package| {
            let mut deps = CrateDeps::default();
            for dep in package.dependencies() {
                let name = dep.crate_name().parse()?;
                let req = VersionReq::parse(dep.requirement())?;

                match dep.kind() {
                    DependencyKind::Normal => deps.main.insert(name, CrateDep::External(req)),
                    DependencyKind::Dev => deps.main.insert(name, CrateDep::External(req)),
                    _ => None,
                };
            }
            let version = Version::parse(package.version())?;
            Ok(CrateRelease {
                name: name.clone(),
                version,
                deps,
                yanked: package.is_yanked(),
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
    index: Index,
}

impl QueryCrate {
    pub fn new(index: Index) -> Self {
        Self { index }
    }

    pub async fn query(index: Index, crate_name: CrateName) -> anyhow::Result<QueryCrateResponse> {
        let crate_name2 = crate_name.clone();
        let krate = spawn_blocking(move || index.crate_(crate_name2.as_ref()))
            .await?
            .ok_or_else(|| anyhow!("crate '{}' not found", crate_name.as_ref()))?;

        convert_pkgs(krate)
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
        let index = self.index.clone();
        Self::query(index, crate_name).boxed()
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

use std::collections::HashSet;
use std::panic::RefUnwindSafe;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cadence::prelude::*;
use cadence::{MetricSink, NopMetricSink, StatsdClient};
use failure::Error;
use futures::future::join_all;
use futures::{future, Future};
use hyper::client::HttpConnector;
use hyper::Client;
use hyper_tls::HttpsConnector;
use relative_path::{RelativePath, RelativePathBuf};
use rustsec::db::AdvisoryDatabase;
use semver::VersionReq;
use slog::Logger;
use tokio_service::Service;

mod futures;
mod machines;

use crate::utils::cache::Cache;

use crate::models::crates::{AnalyzedDependencies, CrateName, CratePath, CrateRelease};
use crate::models::repo::{RepoPath, Repository};

use crate::interactors::crates::{GetPopularCrates, QueryCrate};
use crate::interactors::github::GetPopularRepos;
use crate::interactors::rustsec::FetchAdvisoryDatabase;
use crate::interactors::RetrieveFileAtPath;

use self::futures::AnalyzeDependenciesFuture;
use self::futures::CrawlManifestFuture;

pub type HttpClient = Client<HttpsConnector<HttpConnector>>;

#[derive(Clone, Debug)]
pub struct Engine {
    client: HttpClient,
    logger: Logger,
    metrics: StatsdClient,

    query_crate: Arc<Cache<QueryCrate>>,
    get_popular_crates: Arc<Cache<GetPopularCrates>>,
    get_popular_repos: Arc<Cache<GetPopularRepos>>,
    retrieve_file_at_path: Arc<RetrieveFileAtPath>,
    fetch_advisory_db: Arc<Cache<FetchAdvisoryDatabase>>,
}

impl Engine {
    pub fn new(client: HttpClient, logger: Logger) -> Engine {
        let metrics = StatsdClient::from_sink("engine", NopMetricSink);

        let query_crate = Cache::new(QueryCrate(client.clone()), Duration::from_secs(300), 500);
        let get_popular_crates =
            Cache::new(GetPopularCrates(client.clone()), Duration::from_secs(10), 1);
        let get_popular_repos =
            Cache::new(GetPopularRepos(client.clone()), Duration::from_secs(10), 1);
        let fetch_advisory_db = Cache::new(FetchAdvisoryDatabase {}, Duration::from_secs(300), 1);

        Engine {
            client: client.clone(),
            logger,
            metrics,

            query_crate: Arc::new(query_crate),
            get_popular_crates: Arc::new(get_popular_crates),
            get_popular_repos: Arc::new(get_popular_repos),
            retrieve_file_at_path: Arc::new(RetrieveFileAtPath(client)),
            fetch_advisory_db: Arc::new(fetch_advisory_db),
        }
    }

    pub fn set_metrics<M: MetricSink + Send + Sync + 'static>(&mut self, sink: M) {
        self.metrics = StatsdClient::from_sink("engine", sink);
    }
}

pub struct AnalyzeDependenciesOutcome {
    pub crates: Vec<(CrateName, AnalyzedDependencies)>,
    pub duration: Duration,
}

impl AnalyzeDependenciesOutcome {
    pub fn any_outdated(&self) -> bool {
        self.crates.iter().any(|&(_, ref deps)| deps.any_outdated())
    }

    pub fn any_insecure(&self) -> bool {
        self.crates
            .iter()
            .any(|&(_, ref deps)| deps.count_insecure() > 0)
    }

    pub fn outdated_ratio(&self) -> (usize, usize) {
        self.crates
            .iter()
            .fold((0, 0), |(outdated, total), &(_, ref deps)| {
                (outdated + deps.count_outdated(), total + deps.count_total())
            })
    }
}

impl Engine {
    pub fn get_popular_repos(&self) -> impl Future<Item = Vec<Repository>, Error = Error> + Send {
        self.get_popular_repos.call(()).from_err().map(|repos| {
            repos
                .iter()
                .filter(|repo| !POPULAR_REPOS_BLACKLIST.contains(&repo.path))
                .cloned()
                .collect()
        })
    }

    pub fn get_popular_crates(&self) -> impl Future<Item = Vec<CratePath>, Error = Error> + Send {
        self.get_popular_crates
            .call(())
            .from_err()
            .map(|crates| crates.clone())
    }

    pub fn analyze_repo_dependencies(
        &self,
        repo_path: RepoPath,
    ) -> impl Future<Item = AnalyzeDependenciesOutcome, Error = Error> + Send {
        let start = Instant::now();

        let entry_point = RelativePath::new("/").to_relative_path_buf();
        let manifest_future = CrawlManifestFuture::new(self, repo_path.clone(), entry_point);

        let engine = self.clone();
        manifest_future.and_then(move |manifest_output| {
            let engine_for_analyze = engine.clone();
            let futures = manifest_output
                .crates
                .into_iter()
                .map(move |(crate_name, deps)| {
                    let analyzed_deps_future =
                        AnalyzeDependenciesFuture::new(engine_for_analyze.clone(), deps);

                    analyzed_deps_future.map(move |analyzed_deps| (crate_name, analyzed_deps))
                });

            join_all(futures).and_then(move |crates| {
                let duration = start.elapsed();
                engine
                    .metrics
                    .time_duration_with_tags("analyze_duration", duration)
                    .with_tag("repo_site", repo_path.site.as_ref())
                    .with_tag("repo_qual", repo_path.qual.as_ref())
                    .with_tag("repo_name", repo_path.name.as_ref())
                    .try_send()?;

                Ok(AnalyzeDependenciesOutcome { crates, duration })
            })
        })
    }

    pub fn analyze_crate_dependencies(
        &self,
        crate_path: CratePath,
    ) -> impl Future<Item = AnalyzeDependenciesOutcome, Error = Error> + Send {
        let start = Instant::now();

        let query_future = self.query_crate.call(crate_path.name.clone()).from_err();

        let engine = self.clone();
        query_future.and_then(move |query_response| {
            match query_response
                .releases
                .iter()
                .find(|release| release.version == crate_path.version)
            {
                None => future::Either::A(future::err(format_err!(
                    "could not find crate release with version {}",
                    crate_path.version
                ))),
                Some(release) => {
                    let analyzed_deps_future =
                        AnalyzeDependenciesFuture::new(engine.clone(), release.deps.clone());

                    future::Either::B(analyzed_deps_future.map(move |analyzed_deps| {
                        let crates = vec![(crate_path.name, analyzed_deps)].into_iter().collect();
                        let duration = start.elapsed();

                        AnalyzeDependenciesOutcome { crates, duration }
                    }))
                }
            }
        })
    }

    pub fn find_latest_crate_release(
        &self,
        name: CrateName,
        req: VersionReq,
    ) -> impl Future<Item = Option<CrateRelease>, Error = Error> + Send {
        self.query_crate
            .call(name)
            .from_err()
            .map(move |query_response| {
                query_response
                    .releases
                    .iter()
                    .filter(|release| req.matches(&release.version))
                    .max_by(|r1, r2| r1.version.cmp(&r2.version))
                    .cloned()
            })
    }

    fn fetch_releases<I: IntoIterator<Item = CrateName>>(
        &self,
        names: I,
    ) -> impl Iterator<Item = impl Future<Item = Vec<CrateRelease>, Error = Error> + Send> {
        let engine = self.clone();
        names.into_iter().map(move |name| {
            engine
                .query_crate
                .call(name)
                .from_err()
                .map(|resp| resp.releases.clone())
        })
    }

    fn retrieve_manifest_at_path(
        &self,
        repo_path: &RepoPath,
        path: &RelativePathBuf,
    ) -> impl Future<Item = String, Error = Error> + Send {
        let manifest_path = path.join(RelativePath::new("Cargo.toml"));
        self.retrieve_file_at_path
            .call((repo_path.clone(), manifest_path))
    }

    fn fetch_advisory_db(&self) -> impl Future<Item = Arc<AdvisoryDatabase>, Error = Error> + Send {
        self.fetch_advisory_db
            .call(())
            .from_err()
            .map(|db| db.clone())
    }
}

lazy_static! {
    static ref POPULAR_REPOS_BLACKLIST: HashSet<RepoPath> = {
        vec![
            RepoPath::from_parts("github", "rust-lang", "rust"),
            RepoPath::from_parts("github", "google", "xi-editor"),
            RepoPath::from_parts("github", "lk-geimfari", "awesomo"),
            RepoPath::from_parts("github", "redox-os", "tfs"),
            RepoPath::from_parts("github", "carols10cents", "rustlings"),
            RepoPath::from_parts("github", "rust-unofficial", "awesome-rust"),
        ]
        .into_iter()
        .collect::<Result<HashSet<_>, _>>()
        .unwrap()
    };
}

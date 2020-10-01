use std::{
    collections::HashSet,
    panic::RefUnwindSafe,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Error};
use cadence::{MetricSink, NopMetricSink, StatsdClient};
use futures::{future::try_join_all, stream::FuturesUnordered, Future, FutureExt, Stream};
use hyper::{
    client::{HttpConnector, ResponseFuture},
    service::Service,
    Body, Client, Request, Response,
};
use hyper_tls::HttpsConnector;
use once_cell::sync::Lazy;
use relative_path::{RelativePath, RelativePathBuf};
use rustsec::database::Database;
use semver::VersionReq;
use slog::Logger;

use crate::interactors::crates::{GetPopularCrates, QueryCrate};
use crate::interactors::github::GetPopularRepos;
use crate::interactors::rustsec::FetchAdvisoryDatabase;
use crate::interactors::RetrieveFileAtPath;
use crate::models::crates::{AnalyzedDependencies, CrateName, CratePath, CrateRelease};
use crate::models::repo::{RepoPath, Repository};
use crate::utils::cache::Cache;

mod fut;
mod machines;

use self::fut::analyze_dependencies;
use self::fut::CrawlManifestFuture;

type HttpClient = Client<HttpsConnector<HttpConnector>>;
// type HttpClient = Client<HttpConnector>;

// workaround for hyper 0.12 not implementing Service for Client
#[derive(Debug, Clone)]
struct ServiceHttpClient(HttpClient);

impl Service<Request<Body>> for ServiceHttpClient {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx).map_err(|err| err.into())
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.0.request(req)
    }
}

#[derive(Clone, Debug)]
pub struct Engine {
    client: HttpClient,
    logger: Logger,
    metrics: StatsdClient,
    // TODO: use futures aware mutex
    query_crate: Arc<Mutex<Cache<QueryCrate<ServiceHttpClient>, CrateName>>>,
    get_popular_crates: Arc<Mutex<Cache<GetPopularCrates<ServiceHttpClient>, ()>>>,
    get_popular_repos: Arc<Mutex<Cache<GetPopularRepos<ServiceHttpClient>, ()>>>,
    retrieve_file_at_path: Arc<Mutex<RetrieveFileAtPath<ServiceHttpClient>>>,
    fetch_advisory_db: Arc<Mutex<Cache<FetchAdvisoryDatabase<ServiceHttpClient>, ()>>>,
}

impl Engine {
    pub fn new(client: HttpClient, logger: Logger) -> Engine {
        let metrics = StatsdClient::from_sink("engine", NopMetricSink);

        let service_client = ServiceHttpClient(client.clone());

        let query_crate = Cache::new(
            QueryCrate(service_client.clone()),
            Duration::from_secs(300),
            500,
        );
        let get_popular_crates = Cache::new(
            GetPopularCrates(service_client.clone()),
            Duration::from_secs(10),
            1,
        );
        let get_popular_repos = Cache::new(
            GetPopularRepos(service_client.clone()),
            Duration::from_secs(10),
            1,
        );
        let fetch_advisory_db = Cache::new(
            FetchAdvisoryDatabase(service_client.clone()),
            Duration::from_secs(300),
            1,
        );

        Engine {
            client: client.clone(),
            logger,
            metrics,
            query_crate: Arc::new(Mutex::new(query_crate)),
            get_popular_crates: Arc::new(Mutex::new(get_popular_crates)),
            get_popular_repos: Arc::new(Mutex::new(get_popular_repos)),
            retrieve_file_at_path: Arc::new(Mutex::new(RetrieveFileAtPath(service_client))),
            fetch_advisory_db: Arc::new(Mutex::new(fetch_advisory_db)),
        }
    }

    pub fn set_metrics<M: MetricSink + Send + Sync + RefUnwindSafe + 'static>(&mut self, sink: M) {
        self.metrics = StatsdClient::from_sink("engine", sink);
    }
}

#[derive(Debug)]
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
    pub async fn get_popular_repos(&self) -> Result<Vec<Repository>, Error> {
        let repos = self.get_popular_repos.lock().unwrap().call(());
        let repos = repos.await?;

        let filtered_repos = repos
            .iter()
            .filter(|repo| !POPULAR_REPO_BLOCK_LIST.contains(&repo.path))
            .cloned()
            .collect();

        Ok(filtered_repos)
    }

    pub async fn get_popular_crates(&self) -> Result<Vec<CratePath>, Error> {
        let crates = self.get_popular_crates.lock().unwrap().call(());
        let crates = crates.await?;
        Ok(crates.clone())
    }

    pub async fn analyze_repo_dependencies(
        &self,
        repo_path: RepoPath,
    ) -> Result<AnalyzeDependenciesOutcome, Error> {
        let start = Instant::now();

        let entry_point = RelativePath::new("/").to_relative_path_buf();
        let engine = self.clone();

        let manifest_future = CrawlManifestFuture::new(self, repo_path.clone(), entry_point);
        let manifest_output = manifest_future.await?;

        let engine_for_analyze = engine.clone();
        let futures = manifest_output
            .crates
            .into_iter()
            .map(|(crate_name, deps)| async {
                let analyzed_deps = analyze_dependencies(engine_for_analyze.clone(), deps).await?;
                Ok::<_, Error>((crate_name, analyzed_deps))
            })
            .collect::<Vec<_>>();

        let crates = try_join_all(futures).await?;

        let duration = start.elapsed();
        // engine
        //     .metrics
        //     .time_duration_with_tags("analyze_duration", duration)
        //     .with_tag("repo_site", repo_path.site.as_ref())
        //     .with_tag("repo_qual", repo_path.qual.as_ref())
        //     .with_tag("repo_name", repo_path.name.as_ref())
        //     .send()?;

        Ok(AnalyzeDependenciesOutcome { crates, duration })
    }

    pub async fn analyze_crate_dependencies(
        &self,
        crate_path: CratePath,
    ) -> Result<AnalyzeDependenciesOutcome, Error> {
        let start = Instant::now();

        let query_response = self
            .query_crate
            .lock()
            .unwrap()
            .call(crate_path.name.clone());
        let query_response = query_response.await?;

        let engine = self.clone();

        match query_response
            .releases
            .iter()
            .find(|release| release.version == crate_path.version)
        {
            None => Err(anyhow!(
                "could not find crate release with version {}",
                crate_path.version
            )),

            Some(release) => {
                let analyzed_deps =
                    analyze_dependencies(engine.clone(), release.deps.clone()).await?;

                let crates = vec![(crate_path.name, analyzed_deps)].into_iter().collect();
                let duration = start.elapsed();

                Ok(AnalyzeDependenciesOutcome { crates, duration })
            }
        }
    }

    pub async fn find_latest_crate_release(
        &self,
        name: CrateName,
        req: VersionReq,
    ) -> Result<Option<CrateRelease>, Error> {
        let query_response = self.query_crate.lock().unwrap().call(name);
        let query_response = query_response.await?;

        let latest = query_response
            .releases
            .iter()
            .filter(|release| req.matches(&release.version))
            .max_by(|r1, r2| r1.version.cmp(&r2.version))
            .cloned();

        Ok(latest)
    }

    fn fetch_releases<I: IntoIterator<Item = CrateName>>(
        &self,
        names: I,
    ) -> impl Stream<Item = Result<Vec<CrateRelease>, Error>> {
        let engine = self.clone();

        names
            .into_iter()
            .map(|name| {
                engine
                    .query_crate
                    .lock()
                    .unwrap()
                    .call(name)
                    .map(|resp| resp.map(|r| r.releases.clone()))
            })
            .collect::<FuturesUnordered<_>>()
    }

    fn retrieve_manifest_at_path(
        &self,
        repo_path: &RepoPath,
        path: &RelativePathBuf,
    ) -> impl Future<Output = Result<String, Error>> {
        let manifest_path = path.join(RelativePath::new("Cargo.toml"));

        self.retrieve_file_at_path
            .lock()
            .unwrap()
            .call((repo_path.clone(), manifest_path))
    }

    fn fetch_advisory_db(&self) -> impl Future<Output = Result<Arc<Database>, Error>> {
        self.fetch_advisory_db.lock().unwrap().call(())
    }
}

static POPULAR_REPO_BLOCK_LIST: Lazy<HashSet<RepoPath>> = Lazy::new(|| {
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
});

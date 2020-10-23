use std::{
    collections::HashSet,
    panic::RefUnwindSafe,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Error};
use cadence::{MetricSink, NopMetricSink, StatsdClient};
use futures::{future::try_join_all, stream, StreamExt};
use hyper::service::Service;
use once_cell::sync::Lazy;
use relative_path::{RelativePath, RelativePathBuf};
use rustsec::database::Database;
use semver::VersionReq;
use slog::Logger;
use stream::BoxStream;

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
use self::fut::crawl_manifest;

#[derive(Clone, Debug)]
pub struct Engine {
    client: reqwest::Client,
    logger: Logger,
    metrics: StatsdClient,
    query_crate: Cache<QueryCrate, CrateName>,
    get_popular_crates: Cache<GetPopularCrates, ()>,
    get_popular_repos: Cache<GetPopularRepos, ()>,
    retrieve_file_at_path: RetrieveFileAtPath,
    fetch_advisory_db: Cache<FetchAdvisoryDatabase, ()>,
}

impl Engine {
    pub fn new(client: reqwest::Client, logger: Logger) -> Engine {
        let metrics = StatsdClient::from_sink("engine", NopMetricSink);

        let query_crate = Cache::new(
            QueryCrate::new(client.clone()),
            Duration::from_secs(300),
            500,
            logger.clone(),
        );
        let get_popular_crates = Cache::new(
            GetPopularCrates::new(client.clone()),
            Duration::from_secs(120),
            1,
            logger.clone(),
        );
        let get_popular_repos = Cache::new(
            GetPopularRepos::new(client.clone()),
            Duration::from_secs(120),
            1,
            logger.clone(),
        );
        let retrieve_file_at_path = RetrieveFileAtPath::new(client.clone());
        let fetch_advisory_db = Cache::new(
            FetchAdvisoryDatabase::new(client.clone()),
            Duration::from_secs(1800),
            1,
            logger.clone(),
        );

        Engine {
            client,
            logger,
            metrics,
            query_crate,
            get_popular_crates,
            get_popular_repos,
            retrieve_file_at_path,
            fetch_advisory_db,
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

    // TODO(feliix42): Why is this different from the any_outdated() function above?
    pub fn any_insecure(&self) -> bool {
        self.crates
            .iter()
            .any(|&(_, ref deps)| deps.count_insecure() > 0)
    }

    pub fn any_dev_issues(&self) -> bool {
        self.crates
            .iter()
            .any(|&(_, ref deps)| deps.any_dev_issues())
    }

    pub fn count_dev_outdated(&self) -> usize {
        self.crates
            .iter()
            .map(|&(_, ref deps)| deps.count_dev_outdated())
            .sum()
    }

    pub fn count_dev_insecure(&self) -> usize {
        self.crates
            .iter()
            .map(|&(_, ref deps)| deps.count_dev_insecure())
            .sum()
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
        let repos = self.get_popular_repos.cached_query(()).await?;

        let filtered_repos = repos
            .iter()
            .filter(|repo| !POPULAR_REPO_BLOCK_LIST.contains(&repo.path))
            .cloned()
            .collect();

        Ok(filtered_repos)
    }

    pub async fn get_popular_crates(&self) -> Result<Vec<CratePath>, Error> {
        let crates = self.get_popular_crates.cached_query(()).await?;
        Ok(crates)
    }

    pub async fn analyze_repo_dependencies(
        &self,
        repo_path: RepoPath,
    ) -> Result<AnalyzeDependenciesOutcome, Error> {
        let start = Instant::now();

        let entry_point = RelativePath::new("/").to_relative_path_buf();
        let engine = self.clone();

        let manifest_output = crawl_manifest(self.clone(), repo_path.clone(), entry_point).await?;

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
            .cached_query(crate_path.name.clone())
            .await?;

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

                let crates = vec![(crate_path.name, analyzed_deps)];
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
        let query_response = self.query_crate.cached_query(name).await?;

        let latest = query_response
            .releases
            .iter()
            .filter(|release| req.matches(&release.version))
            .max_by(|r1, r2| r1.version.cmp(&r2.version))
            .cloned();

        Ok(latest)
    }

    fn fetch_releases<'a, I>(&'a self, names: I) -> BoxStream<'a, anyhow::Result<Vec<CrateRelease>>>
    where
        I: IntoIterator<Item = CrateName>,
        <I as IntoIterator>::IntoIter: Send + 'a,
    {
        let engine = self.clone();

        let s = stream::iter(names)
            .zip(stream::repeat(engine))
            .map(resolve_crate_with_engine)
            .buffer_unordered(25);

        Box::pin(s)
    }

    async fn retrieve_manifest_at_path(
        &self,
        repo_path: &RepoPath,
        path: &RelativePathBuf,
    ) -> Result<String, Error> {
        let manifest_path = path.join(RelativePath::new("Cargo.toml"));

        let mut service = self.retrieve_file_at_path.clone();
        Ok(service.call((repo_path.clone(), manifest_path)).await?)
    }

    async fn fetch_advisory_db(&self) -> Result<Arc<Database>, Error> {
        Ok(self.fetch_advisory_db.cached_query(()).await?)
    }
}

async fn resolve_crate_with_engine(
    (crate_name, engine): (CrateName, Engine),
) -> anyhow::Result<Vec<CrateRelease>> {
    let crate_res = engine.query_crate.cached_query(crate_name).await?;
    Ok(crate_res.releases)
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

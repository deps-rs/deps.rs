use std::{
    collections::HashSet,
    panic::RefUnwindSafe,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use actix_web::dev::Service;
use anyhow::{anyhow, Error};
use cadence::{MetricSink, NopMetricSink, StatsdClient};
use futures_util::{
    future::try_join_all,
    stream::{self, LocalBoxStream},
    StreamExt as _,
};
use relative_path::{RelativePath, RelativePathBuf};
use rustsec::database::Database;
use semver::VersionReq;

use crate::{
    interactors::{
        crates::{GetPopularCrates, QueryCrate},
        github::GetPopularRepos,
        rustsec::FetchAdvisoryDatabase,
        RetrieveFileAtPath,
    },
    models::{
        crates::{AnalyzedDependencies, CrateName, CratePath, CrateRelease},
        repo::{RepoPath, Repository},
    },
    utils::cache::Cache,
    ManagedIndex,
};

mod fut;
mod machines;

use self::fut::{analyze_dependencies, crawl_manifest};

#[derive(Debug, Clone)]
pub struct Engine {
    metrics: Arc<StatsdClient>,
    query_crate: Cache<QueryCrate, CrateName>,
    get_popular_crates: Cache<GetPopularCrates, ()>,
    get_popular_repos: Cache<GetPopularRepos, ()>,
    retrieve_file_at_path: RetrieveFileAtPath,
    fetch_advisory_db: Cache<FetchAdvisoryDatabase, ()>,
}

impl Engine {
    pub fn new(client: reqwest::Client, index: ManagedIndex) -> Engine {
        let metrics = Arc::new(StatsdClient::from_sink("engine", NopMetricSink));

        let query_crate = Cache::new(QueryCrate::new(index), Duration::from_secs(10), 500);
        let get_popular_crates = Cache::new(
            GetPopularCrates::new(client.clone()),
            Duration::from_secs(15 * 60),
            1,
        );
        let get_popular_repos = Cache::new(
            GetPopularRepos::new(client.clone()),
            Duration::from_secs(5 * 60),
            1,
        );
        let retrieve_file_at_path = RetrieveFileAtPath::new(client.clone());
        let fetch_advisory_db = Cache::new(
            FetchAdvisoryDatabase::new(client),
            Duration::from_secs(30 * 60),
            1,
        );

        Engine {
            metrics,
            query_crate,
            get_popular_crates,
            get_popular_repos,
            retrieve_file_at_path,
            fetch_advisory_db,
        }
    }

    pub fn set_metrics<M: MetricSink + Send + Sync + RefUnwindSafe + 'static>(&mut self, sink: M) {
        self.metrics = Arc::new(StatsdClient::from_sink("engine", sink));
    }
}

#[derive(Debug)]
pub struct AnalyzeDependenciesOutcome {
    pub crates: Vec<(CrateName, AnalyzedDependencies)>,
    pub duration: Duration,
}

impl AnalyzeDependenciesOutcome {
    pub fn any_outdated(&self) -> bool {
        self.crates.iter().any(|(_, deps)| deps.any_outdated())
    }

    // TODO(feliix42): Why is this different from the any_outdated() function above?
    /// Checks if any insecure main or build dependencies exist in the scanned crates
    pub fn any_insecure(&self) -> bool {
        self.crates
            .iter()
            .any(|(_, deps)| deps.count_insecure() > 0)
    }

    /// Checks if any always insecure main or build dependencies exist in the scanned crates
    pub fn any_always_insecure(&self) -> bool {
        self.crates
            .iter()
            .any(|(_, deps)| deps.count_always_insecure() > 0)
    }

    /// Returns the number of outdated main and dev dependencies
    pub fn count_outdated(&self) -> usize {
        self.crates
            .iter()
            .map(|(_, deps)| deps.count_outdated())
            .sum()
    }

    /// Returns the number of outdated dev-dependencies
    pub fn count_dev_outdated(&self) -> usize {
        self.crates
            .iter()
            .map(|(_, deps)| deps.count_dev_outdated())
            .sum()
    }

    /// Returns the number of insecure dev-dependencies
    pub fn count_dev_insecure(&self) -> usize {
        self.crates
            .iter()
            .map(|(_, deps)| deps.count_dev_insecure())
            .sum()
    }

    /// Returns the number of outdated and the number of total main and build dependencies
    pub fn outdated_ratio(&self) -> (usize, usize) {
        self.crates
            .iter()
            .fold((0, 0), |(outdated, total), (_, deps)| {
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
        sub_path: &Option<String>,
    ) -> Result<AnalyzeDependenciesOutcome, Error> {
        let start = Instant::now();

        let mut entry_point = RelativePath::new("/").to_relative_path_buf();

        if let Some(inner_path) = sub_path {
            entry_point.push(inner_path);
        }

        let engine = self.clone();

        let manifest_output = crawl_manifest(self.clone(), repo_path.clone(), entry_point).await?;

        let futures = manifest_output
            .crates
            .into_iter()
            .map(|(crate_name, deps)| async {
                let analyzed_deps = analyze_dependencies(engine.clone(), deps).await?;
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

    pub async fn find_latest_stable_crate_release(
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

    fn fetch_releases<'a, I>(
        &'a self,
        names: I,
    ) -> LocalBoxStream<'a, anyhow::Result<Vec<CrateRelease>>>
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

        let service = self.retrieve_file_at_path.clone();
        service.call((repo_path.clone(), manifest_path)).await
    }

    async fn fetch_advisory_db(&self) -> Result<Arc<Database>, Error> {
        self.fetch_advisory_db.cached_query(()).await
    }
}

async fn resolve_crate_with_engine(
    (crate_name, engine): (CrateName, Engine),
) -> anyhow::Result<Vec<CrateRelease>> {
    let crate_res = engine.query_crate.cached_query(crate_name).await?;
    Ok(crate_res.releases)
}

static POPULAR_REPO_BLOCK_LIST: LazyLock<HashSet<RepoPath>> = LazyLock::new(|| {
    vec![
        RepoPath::from_parts("github", "rust-lang", "rust"),
        RepoPath::from_parts("github", "xi-editor", "xi-editor"),
        RepoPath::from_parts("github", "lk-geimfari", "awesomo"),
        RepoPath::from_parts("github", "redox-os", "tfs"),
        RepoPath::from_parts("github", "rust-lang", "rustlings"),
        RepoPath::from_parts("github", "rust-unofficial", "awesome-rust"),
        RepoPath::from_parts("github", "996icu", "996.ICU"),
    ]
    .into_iter()
    .collect::<Result<HashSet<_>, _>>()
    .unwrap()
});

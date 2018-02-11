use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use failure::Error;
use futures::Future;
use futures::future::join_all;
use hyper::Client;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use slog::Logger;
use tokio_service::Service;

mod machines;
mod futures;

use ::utils::throttle::Throttle;

use ::models::repo::{Repository, RepoPath};
use ::models::crates::{CrateName, CrateRelease, AnalyzedDependencies};

use ::interactors::crates::query_crate;
use ::interactors::github::retrieve_file_at_path;
use ::interactors::github::GetPopularRepos;

use self::futures::AnalyzeDependenciesFuture;
use self::futures::CrawlManifestFuture;

#[derive(Clone, Debug)]
pub struct Engine {
    client: Client<HttpsConnector<HttpConnector>>,
    logger: Logger,

    get_popular_repos: Arc<Throttle<GetPopularRepos<Client<HttpsConnector<HttpConnector>>>>>
}

impl Engine {
    pub fn new(client: Client<HttpsConnector<HttpConnector>>, logger: Logger) -> Engine {
        Engine {
            client: client.clone(), logger,

            get_popular_repos: Arc::new(Throttle::new(GetPopularRepos(client), Duration::from_secs(10)))
        }
    }
}

pub struct AnalyzeDependenciesOutcome {
    pub crates: Vec<(CrateName, AnalyzedDependencies)>
}

impl AnalyzeDependenciesOutcome {
    pub fn any_outdated(&self) -> bool {
        self.crates.iter().any(|&(_, ref deps)| deps.any_outdated())
    }
}

impl Engine {
    pub fn get_popular_repos(&self) ->
        impl Future<Item=Vec<Repository>, Error=Error>
    {
        self.get_popular_repos.call(())
            .from_err().map(|repos| repos.clone())
    }

    pub fn analyze_dependencies(&self, repo_path: RepoPath) ->
        impl Future<Item=AnalyzeDependenciesOutcome, Error=Error>
    {
        let entry_point = PathBuf::from("/");
        let manifest_future = CrawlManifestFuture::new(self, repo_path, entry_point);

        let engine = self.clone();
        manifest_future.and_then(move |manifest_output| {
            let futures = manifest_output.crates.into_iter().map(move |(crate_name, deps)| {
                let analyzed_deps_future = AnalyzeDependenciesFuture::new(&engine, deps);

                analyzed_deps_future.map(move |analyzed_deps| (crate_name, analyzed_deps))
            });

            join_all(futures).map(|crates| AnalyzeDependenciesOutcome { crates })
        })
    }

    fn fetch_releases<I: IntoIterator<Item=CrateName>>(&self, names: I) ->
        impl Iterator<Item=impl Future<Item=Vec<CrateRelease>, Error=Error>>
    {
        let client = self.client.clone();
        names.into_iter().map(move |name| {
            query_crate(client.clone(), name)
                .map(|resp| resp.releases)
        })
    }

    fn retrieve_manifest_at_path<P: AsRef<Path>>(&self, repo_path: &RepoPath, path: &P) ->
        impl Future<Item=String, Error=Error>
    {
        retrieve_file_at_path(self.client.clone(), &repo_path, &path.as_ref().join("Cargo.toml")).from_err()
    }
}

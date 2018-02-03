mod analyzer;

use futures::{Future, Stream, stream};
use hyper::Client;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use slog::Logger;

use ::models::repo::{Repository, RepoPath};
use ::models::crates::{CrateName, CrateRelease, CrateManifest, AnalyzedDependencies};

use ::parsers::manifest::{ManifestParseError, parse_manifest_toml};

use ::interactors::crates::{QueryCrateError, query_crate};
use ::interactors::github::{RetrieveFileAtPathError, retrieve_file_at_path};
use ::interactors::github::get_popular_repos;
pub use ::interactors::github::GetPopularReposError;

use self::analyzer::DependencyAnalyzer;

#[derive(Clone, Debug)]
pub struct Engine {
    pub client: Client<HttpsConnector<HttpConnector>>,
    pub logger: Logger
}

#[derive(Debug)]
pub enum AnalyzeDependenciesError {
    QueryCrate(QueryCrateError),
    RetrieveFileAtPath(RetrieveFileAtPathError),
    ParseManifest(ManifestParseError)
}

const FETCH_RELEASES_CONCURRENCY: usize = 10;

pub struct AnalyzeDependenciesOutcome {
    pub name: CrateName,
    pub deps: AnalyzedDependencies
} 

impl Engine {
    pub fn get_popular_repos(&self) ->
        impl Future<Item=Vec<Repository>, Error=GetPopularReposError>
    {
        get_popular_repos(self.client.clone())
    }

    pub fn analyze_dependencies(&self, repo_path: RepoPath) ->
        impl Future<Item=AnalyzeDependenciesOutcome, Error=AnalyzeDependenciesError>
    {
        let manifest_future = self.retrieve_manifest(&repo_path);

        let engine = self.clone();
        manifest_future.and_then(move |manifest| {
            let CrateManifest::Crate(crate_name, deps) = manifest;
            let analyzer = DependencyAnalyzer::new(&deps);

            let main_deps = deps.main.into_iter().map(|(name, _)| name);
            let dev_deps = deps.dev.into_iter().map(|(name, _)| name);
            let build_deps = deps.build.into_iter().map(|(name, _)| name);

            let release_futures = engine.fetch_releases(main_deps.chain(dev_deps).chain(build_deps));

            let analyzed_deps_future = stream::iter_ok(release_futures)
                .buffer_unordered(FETCH_RELEASES_CONCURRENCY)
                .fold(analyzer, |mut analyzer, releases| { analyzer.process(releases); Ok(analyzer) })
                .map(|analyzer| analyzer.finalize());

            analyzed_deps_future.map(move |analyzed_deps| {
                AnalyzeDependenciesOutcome {
                    name: crate_name,
                    deps: analyzed_deps
                }
            })
        })
    }

    fn fetch_releases<I: IntoIterator<Item=CrateName>>(&self, names: I) ->
        impl Iterator<Item=impl Future<Item=Vec<CrateRelease>, Error=AnalyzeDependenciesError>>
    {
        let client = self.client.clone();
        names.into_iter().map(move |name| {
            query_crate(client.clone(), name)
                .map_err(AnalyzeDependenciesError::QueryCrate)
                .map(|resp| resp.releases)
        })
    }

    fn retrieve_manifest(&self, repo_path: &RepoPath) ->
        impl Future<Item=CrateManifest, Error=AnalyzeDependenciesError>
    {
        retrieve_file_at_path(self.client.clone(), &repo_path, "Cargo.toml")
            .map_err(AnalyzeDependenciesError::RetrieveFileAtPath)
            .and_then(|manifest_source| {
                parse_manifest_toml(&manifest_source)
                    .map_err(AnalyzeDependenciesError::ParseManifest)
            })
    }
}

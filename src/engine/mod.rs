use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use failure::Error;
use futures::{Future, future};
use futures::future::join_all;
use hyper::Client;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use relative_path::{RelativePath, RelativePathBuf};
use semver::VersionReq;
use slog::Logger;
use tokio_service::Service;

mod machines;
mod futures;

use ::utils::cache::Cache;

use ::models::repo::{Repository, RepoPath};
use ::models::crates::{CrateName, CratePath, CrateRelease, AnalyzedDependencies};

use ::interactors::crates::QueryCrate;
use ::interactors::RetrieveFileAtPath;
use ::interactors::github::{GetPopularRepos};

use self::futures::AnalyzeDependenciesFuture;
use self::futures::CrawlManifestFuture;

type HttpClient = Client<HttpsConnector<HttpConnector>>;

#[derive(Clone, Debug)]
pub struct Engine {
    client: HttpClient,
    logger: Logger,

    query_crate: Arc<Cache<QueryCrate<HttpClient>>>,
    get_popular_repos: Arc<Cache<GetPopularRepos<HttpClient>>>,
    retrieve_file_at_path: Arc<RetrieveFileAtPath<HttpClient>>
}

impl Engine {
    pub fn new(client: Client<HttpsConnector<HttpConnector>>, logger: Logger) -> Engine {
        let query_crate = Cache::new(QueryCrate(client.clone()), Duration::from_secs(300), 500);
        let get_popular_repos = Cache::new(GetPopularRepos(client.clone()), Duration::from_secs(10), 1);

        Engine {
            client: client.clone(), logger,

            query_crate: Arc::new(query_crate),
            get_popular_repos: Arc::new(get_popular_repos),
            retrieve_file_at_path: Arc::new(RetrieveFileAtPath(client))
        }
    }
}

pub struct AnalyzeDependenciesOutcome {
    pub crates: Vec<(CrateName, AnalyzedDependencies)>,
    pub duration: Duration
}

impl AnalyzeDependenciesOutcome {
    pub fn any_outdated(&self) -> bool {
        self.crates.iter().any(|&(_, ref deps)| deps.any_outdated())
    }

    pub fn outdated_ratio(&self) -> (usize, usize) {
        self.crates.iter().fold((0, 0), |(outdated, total), &(_, ref deps)| {
            (outdated + deps.count_outdated(), total + deps.count_total())
        })
    }
}

impl Engine {
    pub fn get_popular_repos(&self) ->
        impl Future<Item=Vec<Repository>, Error=Error>
    {
        self.get_popular_repos.call(())
            .from_err().map(|repos| {
                repos.iter()
                    .filter(|repo| !POPULAR_REPOS_BLACKLIST.contains(&repo.path))
                    .cloned().collect()
            })
    }

    pub fn analyze_repo_dependencies(&self, repo_path: RepoPath) ->
        impl Future<Item=AnalyzeDependenciesOutcome, Error=Error>
    {
        let start = Instant::now();

        let entry_point = RelativePath::new("/").to_relative_path_buf();
        let manifest_future = CrawlManifestFuture::new(self, repo_path, entry_point);

        let engine = self.clone();
        manifest_future.and_then(move |manifest_output| {
            let futures = manifest_output.crates.into_iter().map(move |(crate_name, deps)| {
                let analyzed_deps_future = AnalyzeDependenciesFuture::new(&engine, deps);

                analyzed_deps_future.map(move |analyzed_deps| (crate_name, analyzed_deps))
            });

            join_all(futures).map(move |crates| {
                let duration = start.elapsed();

                AnalyzeDependenciesOutcome {
                    crates, duration
                }
            })
        })
    }

    pub fn analyze_crate_dependencies(&self, crate_path: CratePath) ->
        impl Future<Item=AnalyzeDependenciesOutcome, Error=Error>
    {
        let start = Instant::now();

        let query_future = self.query_crate.call(crate_path.name.clone()).from_err();

        let engine = self.clone();
        query_future.and_then(move |query_response| {
            match query_response.releases.iter().find(|release| release.version == crate_path.version) {
                None => future::Either::A(future::err(format_err!("could not find crate release with version {}", crate_path.version))),
                Some(release) => {
                    let analyzed_deps_future = AnalyzeDependenciesFuture::new(&engine, release.deps.clone());

                    future::Either::B(analyzed_deps_future.map(move |analyzed_deps| {
                        let crates = vec![(crate_path.name, analyzed_deps)].into_iter().collect();
                        let duration = start.elapsed();

                        AnalyzeDependenciesOutcome {
                            crates, duration
                        }
                    }))
                }
            }
        })
    }

    pub fn find_latest_crate_release(&self, name: CrateName, req: Option<VersionReq>) ->
        impl Future<Item=Option<CrateRelease>, Error=Error>
    {
        self.query_crate.call(name).from_err().map(move |query_response| {
            if let Some(vreq) = req {
                query_response.releases.iter()
                    .filter(|release| vreq.matches(&release.version))
                    .max_by(|r1, r2| r1.version.cmp(&r2.version))
                    .cloned()
            } else {
                query_response.releases.iter()
                    .max_by(|r1, r2| r1.version.cmp(&r2.version))
                    .cloned()
            }
        })
    }

    fn fetch_releases<I: IntoIterator<Item=CrateName>>(&self, names: I) ->
        impl Iterator<Item=impl Future<Item=Vec<CrateRelease>, Error=Error>>
    {
        let engine = self.clone();
        names.into_iter().map(move |name| {
            engine.query_crate.call(name)
                .from_err()
                .map(|resp| resp.releases.clone())
        })
    }

    fn retrieve_manifest_at_path(&self, repo_path: &RepoPath, path: &RelativePathBuf) ->
        impl Future<Item=String, Error=Error>
    {
        let manifest_path = path.join(RelativePath::new("Cargo.toml"));
        self.retrieve_file_at_path.call((repo_path.clone(), manifest_path))
    }
}

lazy_static! {
    static ref POPULAR_REPOS_BLACKLIST: HashSet<RepoPath> = {
        vec![
            RepoPath::from_parts("github", "rust-lang", "rust"),
            RepoPath::from_parts("github", "google", "xi-editor"),
            RepoPath::from_parts("github", "lk-geimfari", "awesomo"),
            RepoPath::from_parts("github", "redox-os", "tfs"),
            RepoPath::from_parts("github", "carols10cents", "rustlings")
        ].into_iter().collect::<Result<HashSet<_>, _>>().unwrap()
    };
}

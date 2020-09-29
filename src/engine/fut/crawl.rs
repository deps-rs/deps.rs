use std::mem;

use anyhow::{anyhow, ensure, Error};
use futures::stream::FuturesOrdered;
use futures::{try_ready, Async, Future, Poll, Stream};
use relative_path::RelativePathBuf;

use crate::models::repo::RepoPath;

use super::super::machines::crawler::ManifestCrawler;
pub use super::super::machines::crawler::ManifestCrawlerOutput;
use super::super::Engine;

pub struct CrawlManifestFuture {
    repo_path: RepoPath,
    engine: Engine,
    crawler: ManifestCrawler,
    futures:
        FuturesOrdered<Box<dyn Future<Item = (RelativePathBuf, String), Error = Error> + Send>>,
}

impl CrawlManifestFuture {
    pub fn new(engine: &Engine, repo_path: RepoPath, entry_point: RelativePathBuf) -> Self {
        let future: Box<dyn Future<Item = _, Error = _> + Send> = Box::new(
            engine
                .retrieve_manifest_at_path(&repo_path, &entry_point)
                .map(move |contents| (entry_point, contents)),
        );
        let engine = engine.clone();
        let crawler = ManifestCrawler::new();
        let mut futures = FuturesOrdered::new();
        futures.push(future);

        CrawlManifestFuture {
            repo_path,
            engine,
            crawler,
            futures,
        }
    }
}

impl Future for CrawlManifestFuture {
    type Item = ManifestCrawlerOutput;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match try_ready!(self.futures.poll()) {
            None => {
                let crawler = mem::replace(&mut self.crawler, ManifestCrawler::new());
                Ok(Async::Ready(crawler.finalize()))
            }
            Some((path, raw_manifest)) => {
                let output = self.crawler.step(path, raw_manifest)?;
                for path in output.paths_of_interest.into_iter() {
                    let future: Box<dyn Future<Item = _, Error = _> + Send> = Box::new(
                        self.engine
                            .retrieve_manifest_at_path(&self.repo_path, &path)
                            .map(move |contents| (path, contents)),
                    );
                    self.futures.push(future);
                }
                self.poll()
            }
        }
    }
}

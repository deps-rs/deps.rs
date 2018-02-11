use std::mem;
use std::path::PathBuf;

use failure::Error;
use futures::{Async, Future, Poll, Stream};
use futures::stream::FuturesUnordered;

use ::models::repo::RepoPath;

use super::super::Engine;
use super::super::machines::crawler::ManifestCrawler;
pub use super::super::machines::crawler::ManifestCrawlerOutput;

pub struct CrawlManifestFuture {
    repo_path: RepoPath,
    engine: Engine,
    crawler: ManifestCrawler,
    unordered: FuturesUnordered<Box<Future<Item=(PathBuf, String), Error=Error>>>
}

impl CrawlManifestFuture {
    pub fn new(engine: &Engine, repo_path: RepoPath, entry_point: PathBuf) -> Self {
        let future: Box<Future<Item=_, Error=_>> = Box::new(engine.retrieve_manifest_at_path(&repo_path, &entry_point)
            .map(move |contents| (entry_point, contents)));
        let engine = engine.clone();
        let crawler = ManifestCrawler::new();
        let mut unordered = FuturesUnordered::new();
        unordered.push(future);

        CrawlManifestFuture {
            repo_path, engine, crawler, unordered
        }
    }
}

impl Future for CrawlManifestFuture {
    type Item = ManifestCrawlerOutput;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match try_ready!(self.unordered.poll()) {
            None => {
                let crawler = mem::replace(&mut self.crawler, ManifestCrawler::new());
                Ok(Async::Ready(crawler.finalize()))
            },
            Some((path, raw_manifest)) => {
                let output = self.crawler.step(path, raw_manifest)?;
                for path in output.paths_of_interest.into_iter() {
                    let future: Box<Future<Item=_, Error=_>> = Box::new(self.engine.retrieve_manifest_at_path(&self.repo_path, &path)
                        .map(move |contents| (path, contents)));
                    self.unordered.push(future);
                }
                self.poll()
            }
        }
    }
}

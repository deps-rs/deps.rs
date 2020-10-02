use std::{future::Future, mem, pin::Pin, task::Context, task::Poll};

use anyhow::Error;
use futures::{future::BoxFuture, ready, Stream};
use futures::{stream::FuturesOrdered, FutureExt};
use relative_path::RelativePathBuf;

use crate::models::repo::RepoPath;

use super::super::machines::crawler::ManifestCrawler;
pub use super::super::machines::crawler::ManifestCrawlerOutput;
use super::super::Engine;

#[pin_project::pin_project]
pub struct CrawlManifestFuture {
    repo_path: RepoPath,
    engine: Engine,
    crawler: ManifestCrawler,
    #[pin]
    futures: FuturesOrdered<BoxFuture<'static, Result<(RelativePathBuf, String), Error>>>,
}

impl CrawlManifestFuture {
    pub fn new(engine: &Engine, repo_path: RepoPath, entry_point: RelativePathBuf) -> Self {
        let engine = engine.clone();
        let crawler = ManifestCrawler::new();
        let mut futures = FuturesOrdered::new();

        let future: Pin<Box<dyn Future<Output = _> + Send>> = Box::pin(
            engine
                .retrieve_manifest_at_path(&repo_path, &entry_point)
                .map(move |contents| contents.map(|c| (entry_point, c))),
        );
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
    type Output = Result<ManifestCrawlerOutput, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();

        match ready!(this.futures.poll_next(cx)) {
            None => {
                let crawler = mem::replace(&mut self.crawler, ManifestCrawler::new());
                Poll::Ready(Ok(crawler.finalize()))
            }

            Some(Ok((path, raw_manifest))) => {
                let output = self.crawler.step(path, raw_manifest)?;

                for path in output.paths_of_interest.into_iter() {
                    let future: Pin<Box<dyn Future<Output = _> + Send>> = Box::pin(
                        self.engine
                            .retrieve_manifest_at_path(&self.repo_path, &path)
                            .map(move |contents| contents.map(|c| (path, c))),
                    );
                    self.futures.push(future);
                }

                self.poll(cx)
            }

            Some(Err(err)) => Poll::Ready(Err(err)),
        }
    }
}

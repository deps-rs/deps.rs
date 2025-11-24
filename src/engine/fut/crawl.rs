use anyhow::Error;
use futures_util::{
    FutureExt as _, StreamExt as _, future::LocalBoxFuture, stream::FuturesOrdered,
};
use relative_path::RelativePathBuf;

use crate::{
    engine::{
        Engine,
        machines::crawler::{ManifestCrawler, ManifestCrawlerOutput},
    },
    models::repo::RepoPath,
};

pub async fn crawl_manifest(
    engine: Engine,
    repo_path: RepoPath,
    entry_point: RelativePathBuf,
) -> anyhow::Result<ManifestCrawlerOutput> {
    let mut crawler = ManifestCrawler::new();
    let mut futures: FuturesOrdered<
        LocalBoxFuture<'static, Result<(RelativePathBuf, String), Error>>,
    > = FuturesOrdered::new();

    let engine2 = engine.clone();
    let repo_path2 = repo_path.clone();

    let fut = async move {
        let contents = engine2
            .retrieve_manifest_at_path(&repo_path2, &entry_point)
            .await?;
        Ok((entry_point, contents))
    }
    .boxed_local();

    futures.push_back(fut);

    while let Some(item) = futures.next().await {
        let (path, raw_manifest) = item?;
        let output = crawler.step(path, raw_manifest)?;

        let engine = engine.clone();
        let repo_path = repo_path.clone();

        for path in output.paths_of_interest {
            let engine = engine.clone();
            let repo_path = repo_path.clone();

            let fut = async move {
                let contents = engine.retrieve_manifest_at_path(&repo_path, &path).await?;
                Ok((path, contents))
            }
            .boxed_local();

            futures.push_back(fut);
        }
    }

    Ok(crawler.finalize())
}

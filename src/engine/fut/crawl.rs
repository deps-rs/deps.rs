use anyhow::Error;
use futures_util::{future::BoxFuture, stream::FuturesOrdered, FutureExt as _, StreamExt as _};
use relative_path::RelativePathBuf;

use crate::models::repo::RepoPath;

use crate::engine::{
    machines::crawler::{ManifestCrawler, ManifestCrawlerOutput},
    Engine,
};

pub async fn crawl_manifest(
    engine: Engine,
    repo_path: RepoPath,
    entry_point: RelativePathBuf,
) -> anyhow::Result<ManifestCrawlerOutput> {
    let mut crawler = ManifestCrawler::new();
    let mut futures: FuturesOrdered<BoxFuture<'static, Result<(RelativePathBuf, String), Error>>> =
        FuturesOrdered::new();

    // cargo toml
    let engine2 = engine.clone();
    let repo_path2 = repo_path.clone();
    let entry_point2 = entry_point.clone();

    let fut = async move {
        let contents = engine2
            .retrieve_manifest_at_path(&repo_path2, &entry_point2)
            .await?;
        Ok((entry_point2, contents))
    }
    .boxed();

    futures.push_back(fut);

    // cargo toml: paths_of_interest
    while let Some(item) = futures.next().await {
        let (path, raw_manifest) = item?;
        let output = crawler.step(path, raw_manifest)?;

        for path in output.paths_of_interest {
            let engine = engine.clone();
            let repo_path = repo_path.clone();

            let fut = async move {
                let contents = engine.retrieve_manifest_at_path(&repo_path, &path).await?;
                Ok((path, contents))
            }
            .boxed();

            futures.push_back(fut);
        }
    }

    // cargo lock
    let engine3 = engine.clone();
    let repo_path3 = repo_path.clone();
    let entry_point3 = entry_point.clone();

    let contents = engine3
        .retrieve_lock_at_path(&repo_path3, &entry_point3)
        .await;
    if contents.is_ok() {
        _ = crawler.process_lock(entry_point3, contents.unwrap());
    }

    Ok(crawler.finalize())
}

use anyhow::Error;
use hyper::Uri;
use relative_path::RelativePathBuf;

use crate::models::repo::RepoPath;

const BITBUCKET_USER_CONTENT_BASE_URI: &'static str = "https://bitbucket.org";

pub fn get_manifest_uri(repo_path: &RepoPath, path: &RelativePathBuf) -> Result<Uri, Error> {
    let path_str: &str = path.as_ref();

    let url = format!(
        "{}/{}/{}/raw/HEAD/{}",
        BITBUCKET_USER_CONTENT_BASE_URI,
        repo_path.qual.as_ref(),
        repo_path.name.as_ref(),
        path_str
    );

    Ok(url.parse::<Uri>()?)
}

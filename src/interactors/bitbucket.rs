use hyper::Uri;
use relative_path::RelativePathBuf;

use ::models::repo::RepoPath;

const BITBUCKET_USER_CONTENT_BASE_URI: &'static str = "https://bitbucket.org";

pub fn get_manifest_uri(repo_path: &RepoPath, path: &RelativePathBuf) -> Result<Uri, ::hyper::error::UriError> {
    let path_str: &str = path.as_ref();
    format!("{}/{}/{}/raw/master/{}",
        BITBUCKET_USER_CONTENT_BASE_URI,
        repo_path.qual.as_ref(),
        repo_path.name.as_ref(),
        path_str
    ).parse::<Uri>()
}


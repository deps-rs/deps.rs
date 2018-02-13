use hyper::Uri;
use relative_path::RelativePathBuf;
use failure::Error;

use ::models::repo::RepoPath;

const GITLAB_USER_CONTENT_BASE_URI: &'static str = "https://gitlab.com";

pub fn get_manifest_uri(repo_path: &RepoPath, path: &RelativePathBuf) -> Result<Uri, Error> {
    let path_str: &str = path.as_ref();
    // gitlab will return a 308 if the Uri ends with, say, `.../raw/HEAD//Cargo.toml`, so make
    // sure that last slash isn't doubled
    let slash_path = if path_str.starts_with("/") {
        &path_str[1..]
    } else {
        path_str
    };
    Ok(format!("{}/{}/{}/raw/HEAD/{}",
        GITLAB_USER_CONTENT_BASE_URI,
        repo_path.qual.as_ref(),
        repo_path.name.as_ref(),
        slash_path
    ).parse::<Uri>()?)
}

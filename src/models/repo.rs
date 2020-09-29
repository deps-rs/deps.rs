use std::str::FromStr;

use anyhow::{anyhow, ensure, Error};

#[derive(Clone, Debug)]
pub struct Repository {
    pub path: RepoPath,
    pub description: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RepoPath {
    pub site: RepoSite,
    pub qual: RepoQualifier,
    pub name: RepoName,
}

impl RepoPath {
    pub fn from_parts(site: &str, qual: &str, name: &str) -> Result<RepoPath, Error> {
        Ok(RepoPath {
            site: site.parse()?,
            qual: qual.parse()?,
            name: name.parse()?,
        })
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum RepoSite {
    Github,
    Gitlab,
    Bitbucket,
}

impl RepoSite {
    pub fn to_base_uri(&self) -> &'static str {
        match self {
            &RepoSite::Github => "https://github.com",
            &RepoSite::Gitlab => "https://gitlab.com",
            &RepoSite::Bitbucket => "https://bitbucket.org",
        }
    }
}

impl FromStr for RepoSite {
    type Err = Error;

    fn from_str(input: &str) -> Result<RepoSite, Error> {
        match input {
            "github" => Ok(RepoSite::Github),
            "gitlab" => Ok(RepoSite::Gitlab),
            "bitbucket" => Ok(RepoSite::Bitbucket),
            _ => Err(anyhow!("unknown repo site identifier")),
        }
    }
}

impl AsRef<str> for RepoSite {
    fn as_ref(&self) -> &str {
        match self {
            &RepoSite::Github => "github",
            &RepoSite::Gitlab => "gitlab",
            &RepoSite::Bitbucket => "bitbucket",
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RepoQualifier(String);

impl FromStr for RepoQualifier {
    type Err = Error;

    fn from_str(input: &str) -> Result<RepoQualifier, Error> {
        let is_valid = input
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');

        ensure!(is_valid, "invalid repo qualifier");
        Ok(RepoQualifier(input.to_string()))
    }
}

impl AsRef<str> for RepoQualifier {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RepoName(String);

impl FromStr for RepoName {
    type Err = Error;

    fn from_str(input: &str) -> Result<RepoName, Error> {
        let is_valid = input
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');

        ensure!(is_valid, "invalid repo name");
        Ok(RepoName(input.to_string()))
    }
}

impl AsRef<str> for RepoName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

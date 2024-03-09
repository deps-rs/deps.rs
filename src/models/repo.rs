use std::{fmt, str::FromStr};

use anyhow::{anyhow, ensure, Error};
use relative_path::RelativePath;

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

    pub fn to_usercontent_file_url(&self, path: &RelativePath) -> String {
        format!(
            "{}/{}/{}/{}/{}",
            self.site.to_usercontent_base_uri(),
            self.qual.as_ref(),
            self.name.as_ref(),
            self.site.to_usercontent_repo_suffix(),
            path.normalize()
        )
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} => {}/{}",
            self.site,
            self.qual.as_ref(),
            self.name.as_ref()
        )
    }
}

#[allow(clippy::similar_names)]
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum RepoSite {
    Github,
    Gitlab(Option<GiteaDomain>),
    Bitbucket,
    Sourcehut,
    Codeberg,
    Gitea(GiteaDomain),
}

impl RepoSite {
    pub fn to_base_uri(&self) -> &str {
        match self {
            RepoSite::Github => "https://github.com",
            RepoSite::Gitlab(None) => "https://gitlab.com",
            RepoSite::Gitlab(Some(domain)) => domain.as_ref(),
            RepoSite::Bitbucket => "https://bitbucket.org",
            RepoSite::Sourcehut => "https://git.sr.ht",
            RepoSite::Codeberg => "https://codeberg.org",
            RepoSite::Gitea(domain) => domain.as_ref(),
        }
    }

    pub fn to_usercontent_base_uri(&self) -> &str {
        match self {
            RepoSite::Github => "https://raw.githubusercontent.com",
            RepoSite::Gitlab(None) => "https://gitlab.com",
            RepoSite::Gitlab(Some(domain)) => domain.as_ref(),
            RepoSite::Bitbucket => "https://bitbucket.org",
            RepoSite::Sourcehut => "https://git.sr.ht",
            RepoSite::Codeberg => "https://codeberg.org",
            RepoSite::Gitea(domain) => domain.as_ref(),
        }
    }

    pub fn to_usercontent_repo_suffix(&self) -> &'static str {
        match self {
            RepoSite::Github => "HEAD",
            RepoSite::Gitlab(_) | RepoSite::Bitbucket => "raw/HEAD",
            RepoSite::Sourcehut => "blob/HEAD",
            RepoSite::Codeberg | RepoSite::Gitea(_) => "raw",
        }
    }
}

impl FromStr for RepoSite {
    type Err = Error;

    fn from_str(input: &str) -> Result<RepoSite, Error> {
        if let Some((site, domain)) = input.split_once('/') {
            match site {
                "gitea" => Ok(RepoSite::Gitea(domain.parse()?)),
                "gitlab" => Ok(RepoSite::Gitlab(Some(domain.parse()?))),
                _ => Err(anyhow!("unknown repo site identifier")),
            }
        } else {
            match input {
                "github" => Ok(RepoSite::Github),
                "gitlab" => Ok(RepoSite::Gitlab(None)),
                "bitbucket" => Ok(RepoSite::Bitbucket),
                "sourcehut" => Ok(RepoSite::Sourcehut),
                "codeberg" => Ok(RepoSite::Codeberg),
                _ => Err(anyhow!("unknown repo site identifier")),
            }
        }
    }
}

impl fmt::Display for RepoSite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepoSite::Github => write!(f, "github"),
            RepoSite::Gitlab(None) => write!(f, "gitlab"),
            RepoSite::Gitlab(Some(s)) => write!(f, "gitlab/{s}"),
            RepoSite::Bitbucket => write!(f, "bitbucket"),
            RepoSite::Sourcehut => write!(f, "sourcehut"),
            RepoSite::Codeberg => write!(f, "codeberg"),
            RepoSite::Gitea(s) => write!(f, "gitea/{s}"),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GiteaDomain(String);

impl FromStr for GiteaDomain {
    type Err = Error;

    fn from_str(input: &str) -> Result<GiteaDomain, Error> {
        if input.starts_with("https://") || input.starts_with("http://") {
            Ok(GiteaDomain(input.to_string()))
        } else {
            Ok(GiteaDomain(format!("https://{input}")))
        }
    }
}

impl AsRef<str> for GiteaDomain {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl fmt::Display for GiteaDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.starts_with("https://") {
            f.write_str(&self.0["https://".len()..])
        } else {
            self.0.fmt(f)
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RepoQualifier(String);

impl FromStr for RepoQualifier {
    type Err = Error;

    fn from_str(input: &str) -> Result<RepoQualifier, Error> {
        let is_valid = input.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'
                 // Sourcehut projects have the form
                 // https://git.sr.ht/~user/project.
                 || c == '~'
        });

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_raw_url_generation() {
        let paths = [
            ("Cargo.toml", "Cargo.toml"),
            ("/Cargo.toml", "Cargo.toml"),
            ("libs/badge/Cargo.toml", "libs/badge/Cargo.toml"),
            ("/libs/badge/Cargo.toml", "libs/badge/Cargo.toml"),
            ("src/../libs/badge/Cargo.toml", "libs/badge/Cargo.toml"),
            ("/src/../libs/badge/Cargo.toml", "libs/badge/Cargo.toml"),
        ];

        for (input, expected) in &paths {
            let repo = RepoPath::from_parts("github", "deps-rs", "deps.rs").unwrap();
            let out = repo.to_usercontent_file_url(RelativePath::new(input));

            let exp = format!("https://raw.githubusercontent.com/deps-rs/deps.rs/HEAD/{expected}");
            assert_eq!(out.to_string(), exp);
        }

        for (input, expected) in &paths {
            let repo = RepoPath::from_parts("gitlab", "deps-rs", "deps.rs").unwrap();
            let out = repo.to_usercontent_file_url(RelativePath::new(input));

            let exp = format!("https://gitlab.com/deps-rs/deps.rs/raw/HEAD/{expected}");
            assert_eq!(out.to_string(), exp);
        }

        for (input, expected) in &paths {
            let repo = RepoPath::from_parts("bitbucket", "deps-rs", "deps.rs").unwrap();
            let out = repo.to_usercontent_file_url(RelativePath::new(input));

            let exp = format!("https://bitbucket.org/deps-rs/deps.rs/raw/HEAD/{expected}");
            assert_eq!(out.to_string(), exp);
        }

        for (input, expected) in &paths {
            let repo = RepoPath::from_parts("codeberg", "deps-rs", "deps.rs").unwrap();
            let out = repo.to_usercontent_file_url(RelativePath::new(input));

            let exp = format!("https://codeberg.org/deps-rs/deps.rs/raw/{expected}");
            assert_eq!(out.to_string(), exp);
        }

        for (input, expected) in &paths {
            let repo = RepoPath::from_parts("gitea/gitea.com", "deps-rs", "deps.rs").unwrap();
            let out = repo.to_usercontent_file_url(RelativePath::new(input));

            let exp = format!("https://gitea.com/deps-rs/deps.rs/raw/{expected}");
            assert_eq!(out.to_string(), exp);
        }

        for (input, expected) in &paths {
            let repo = RepoPath::from_parts("gitea/example.com/git", "deps-rs", "deps.rs").unwrap();
            let out = repo.to_usercontent_file_url(RelativePath::new(input));

            let exp = format!("https://example.com/git/deps-rs/deps.rs/raw/{expected}");
            assert_eq!(out.to_string(), exp);
        }

        for (input, expected) in &paths {
            let repo = RepoPath::from_parts("gitlab/gitlab.com", "deps-rs", "deps.rs").unwrap();
            let out = repo.to_usercontent_file_url(RelativePath::new(input));

            let exp = format!("https://gitlab.com/deps-rs/deps.rs/raw/HEAD/{expected}");
            assert_eq!(out.to_string(), exp);
        }
    }
}

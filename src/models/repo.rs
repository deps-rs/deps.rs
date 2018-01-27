use std::str::FromStr;

#[derive(Clone)]
pub struct RepoPath {
    pub site: RepoSite,
    pub qual: RepoQualifier,
    pub name: RepoName
}

impl RepoPath {
    pub fn from_parts(site: &str, qual: &str, name: &str) -> Result<RepoPath, RepoValidationError> {
        Ok(RepoPath {
            site: site.parse()?,
            qual: qual.parse()?,
            name: name.parse()?
        })
    }
}

#[derive(Debug)]
pub struct RepoValidationError;

#[derive(Clone, Copy)]
pub enum RepoSite {
    Github
}

impl RepoSite {
    pub fn to_base_uri(&self) -> &'static str {
        match self {
            &RepoSite::Github => "https://github.com"
        }
    }
}

impl FromStr for RepoSite {
    type Err = RepoValidationError;

    fn from_str(input: &str) -> Result<RepoSite, RepoValidationError> {
        match input {
            "github" => Ok(RepoSite::Github),
            _ => Err(RepoValidationError)
        }
    }
}

impl AsRef<str> for RepoSite {
    fn as_ref(&self) -> &str {
        match self {
            &RepoSite::Github => "github"
        }
    }
}

#[derive(Clone)]
pub struct RepoQualifier(String);

impl FromStr for RepoQualifier {
    type Err = RepoValidationError;

    fn from_str(input: &str) -> Result<RepoQualifier, RepoValidationError> {
        let is_valid = input.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'
        });

        if !is_valid {
            Err(RepoValidationError)
        } else {
            Ok(RepoQualifier(input.to_string()))
        }
    }
}

impl AsRef<str> for RepoQualifier {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Clone)]
pub struct RepoName(String);

impl FromStr for RepoName {
    type Err = RepoValidationError;

    fn from_str(input: &str) -> Result<RepoName, RepoValidationError> {
        let is_valid = input.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'
        });

        if !is_valid {
            Err(RepoValidationError)
        } else {
            Ok(RepoName(input.to_string()))
        }
    }
}

impl AsRef<str> for RepoName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

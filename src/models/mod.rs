pub mod crates;
pub mod repo;

pub enum SubjectPath {
    Repo(self::repo::RepoPath),
    Crate(self::crates::CratePath),
}

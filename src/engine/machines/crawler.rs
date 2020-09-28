use std::collections::HashMap;

use anyhow::{anyhow, ensure, Error};
use indexmap::IndexMap;
use relative_path::RelativePathBuf;

use crate::models::crates::{CrateDep, CrateDeps, CrateManifest, CrateName};
use crate::parsers::manifest::parse_manifest_toml;

pub struct ManifestCrawlerOutput {
    pub crates: IndexMap<CrateName, CrateDeps>,
}

pub struct ManifestCrawlerStepOutput {
    pub paths_of_interest: Vec<RelativePathBuf>,
}

pub struct ManifestCrawler {
    manifests: HashMap<RelativePathBuf, CrateManifest>,
    leaf_crates: IndexMap<CrateName, CrateDeps>,
}

impl ManifestCrawler {
    pub fn new() -> ManifestCrawler {
        ManifestCrawler {
            manifests: HashMap::new(),
            leaf_crates: IndexMap::new(),
        }
    }

    pub fn step(
        &mut self,
        path: RelativePathBuf,
        raw_manifest: String,
    ) -> Result<ManifestCrawlerStepOutput, Error> {
        let manifest = parse_manifest_toml(&raw_manifest)?;
        self.manifests.insert(path.clone(), manifest.clone());

        let mut output = ManifestCrawlerStepOutput {
            paths_of_interest: vec![],
        };

        match manifest {
            CrateManifest::Package(name, deps) => {
                self.process_package(&path, name, deps, &mut output);
            }
            CrateManifest::Workspace { members } => {
                self.process_workspace(&path, &members, &mut output);
            }
            CrateManifest::Mixed {
                name,
                deps,
                members,
            } => {
                self.process_package(&path, name, deps, &mut output);
                self.process_workspace(&path, &members, &mut output);
            }
        }

        Ok(output)
    }

    fn register_interest(
        &mut self,
        base_path: &RelativePathBuf,
        path: &RelativePathBuf,
        output: &mut ManifestCrawlerStepOutput,
    ) {
        let full_path = base_path.join_normalized(path);
        if !self.manifests.contains_key(&full_path) {
            output.paths_of_interest.push(full_path);
        }
    }

    fn process_package(
        &mut self,
        base_path: &RelativePathBuf,
        name: CrateName,
        deps: CrateDeps,
        output: &mut ManifestCrawlerStepOutput,
    ) {
        for (_, dep) in deps
            .main
            .iter()
            .chain(deps.dev.iter())
            .chain(deps.build.iter())
        {
            if let &CrateDep::Internal(ref path) = dep {
                self.register_interest(base_path, path, output);
            }
        }

        self.leaf_crates.insert(name, deps);
    }

    fn process_workspace(
        &mut self,
        base_path: &RelativePathBuf,
        members: &[RelativePathBuf],
        output: &mut ManifestCrawlerStepOutput,
    ) {
        for path in members {
            if !path.ends_with("*") {
                self.register_interest(base_path, path, output);
            }
        }
    }

    pub fn finalize(self) -> ManifestCrawlerOutput {
        ManifestCrawlerOutput {
            crates: self.leaf_crates,
        }
    }
}

#[cfg(test)]
mod tests {
    use relative_path::RelativePath;
    use semver::VersionReq;

    use crate::models::crates::CrateDep;

    use super::*;

    #[test]
    fn simple_package_manifest() {
        let manifest = r#"
[package]
name = "simpleton"
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler
            .step("Cargo.toml".into(), manifest.to_string())
            .unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 0);
        let output = crawler.finalize();
        assert_eq!(output.crates.len(), 1);
        assert_eq!(output.crates["simpleton"].main.len(), 0);
        assert_eq!(output.crates["simpleton"].dev.len(), 0);
        assert_eq!(output.crates["simpleton"].build.len(), 0);
    }

    #[test]
    fn more_complex_package_manifest() {
        let manifest = r#"
[package]
name = "more-complex"
[dependencies]
foo = "0.30.0"
bar = { version = "1.2.0", optional = true }
[dev-dependencies]
quickcheck = "0.5"
[build-dependencies]
codegen = "0.0.1"
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 0);
        let output = crawler.finalize();
        assert_eq!(output.crates.len(), 1);
        assert_eq!(output.crates["more-complex"].main.len(), 2);
        assert_eq!(
            output.crates["more-complex"].main.get("foo").unwrap(),
            &CrateDep::External(VersionReq::parse("0.30.0").unwrap())
        );
        assert_eq!(
            output.crates["more-complex"].main.get("bar").unwrap(),
            &CrateDep::External(VersionReq::parse("1.2.0").unwrap())
        );
        assert_eq!(output.crates["more-complex"].dev.len(), 1);
        assert_eq!(
            output.crates["more-complex"].dev.get("quickcheck").unwrap(),
            &CrateDep::External(VersionReq::parse("0.5").unwrap())
        );
        assert_eq!(output.crates["more-complex"].build.len(), 1);
        assert_eq!(
            output.crates["more-complex"].build.get("codegen").unwrap(),
            &CrateDep::External(VersionReq::parse("0.0.1").unwrap())
        );
    }

    #[test]
    fn package_manifest_with_internal_dependencies() {
        let manifest = r#"
[package]
name = "piston"

[dependencies.pistoncore-input]
path = "src/input"
version = "0.20.0"

[dependencies.pistoncore-window]
path = "src/window"
version = "0.30.0"

[dependencies.pistoncore-event_loop]
path = "src/event_loop"
version = "0.35.0"
"#;

        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 3);
        assert_eq!(step_output.paths_of_interest[0].as_str(), "src/input");
        assert_eq!(step_output.paths_of_interest[1].as_str(), "src/window");
        assert_eq!(step_output.paths_of_interest[2].as_str(), "src/event_loop");
    }

    #[test]
    fn simple_workspace_manifest() {
        let manifest = r#"
[workspace]
members = [
  "lib/",
  "codegen/",
  "contrib/",
]
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 3);
        assert_eq!(step_output.paths_of_interest[0].as_str(), "lib");
        assert_eq!(step_output.paths_of_interest[1].as_str(), "codegen");
        assert_eq!(step_output.paths_of_interest[2].as_str(), "contrib");
    }

    #[test]
    fn glob_workspace_manifest() {
        let manifest = r#"
[workspace]
members = [
  "lib/",
  "tests/*",
]
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 1);
        assert_eq!(step_output.paths_of_interest[0].as_str(), "lib");
    }

    #[test]
    fn mixed_package_and_workspace_manifest() {
        let futures_manifest = r#"
[package]
name = "futures"

[dependencies]

[workspace]
members = ["futures-cpupool"]
"#;

        let futures_cpupool_manifest = r#"
[package]
name = "futures-cpupool"

[dependencies]
num_cpus = "1.0"

[dependencies.futures]
path = ".."
version = "0.1"
default-features = false
features = ["use_std"]
"#;

        let mut crawler = ManifestCrawler::new();
        let step_output = crawler
            .step("".into(), futures_manifest.to_string())
            .unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 1);
        assert_eq!(step_output.paths_of_interest[0].as_str(), "futures-cpupool");
        let step_output = crawler
            .step(
                "futures-cpupool".into(),
                futures_cpupool_manifest.to_string(),
            )
            .unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 0);
        let output = crawler.finalize();
        assert_eq!(output.crates.len(), 2);
        assert_eq!(output.crates["futures"].main.len(), 0);
        assert_eq!(output.crates["futures"].dev.len(), 0);
        assert_eq!(output.crates["futures"].build.len(), 0);
        assert_eq!(output.crates["futures-cpupool"].main.len(), 2);
        assert_eq!(
            output.crates["futures-cpupool"]
                .main
                .get("num_cpus")
                .unwrap(),
            &CrateDep::External(VersionReq::parse("1.0").unwrap())
        );
        assert_eq!(
            output.crates["futures-cpupool"]
                .main
                .get("futures")
                .unwrap(),
            &CrateDep::Internal(RelativePath::new("..").to_relative_path_buf())
        );
        assert_eq!(output.crates["futures-cpupool"].dev.len(), 0);
        assert_eq!(output.crates["futures-cpupool"].build.len(), 0);
    }
}

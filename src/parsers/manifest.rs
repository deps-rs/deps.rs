use std::str::FromStr;

use anyhow::{anyhow, Error};
use cargo_lock::{Lockfile, Package};
use indexmap::IndexMap;
use relative_path::RelativePathBuf;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::models::crates::{CrateDep, CrateDeps, CrateManifest, CrateName};

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlComplexDependency {
    git: Option<String>,
    path: Option<RelativePathBuf>,
    version: Option<String>,
    package: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum CargoTomlDependency {
    Simple(String),
    Complex(CargoTomlComplexDependency),
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlPackage {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlWorkspace {
    #[serde(default)]
    members: Vec<RelativePathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoToml {
    #[serde(default)]
    package: Option<CargoTomlPackage>,
    #[serde(default)]
    workspace: Option<CargoTomlWorkspace>,
    #[serde(default)]
    dependencies: IndexMap<String, CargoTomlDependency>,
    #[serde(rename = "dev-dependencies")]
    #[serde(default)]
    dev_dependencies: IndexMap<String, CargoTomlDependency>,
    #[serde(rename = "build-dependencies")]
    #[serde(default)]
    build_dependencies: IndexMap<String, CargoTomlDependency>,
}

fn convert_dependency(
    cargo_dep: (String, CargoTomlDependency),
) -> Option<Result<(CrateName, CrateDep), Error>> {
    match cargo_dep {
        (name, CargoTomlDependency::Simple(string)) => {
            Some(name.parse::<CrateName>().and_then(|parsed_name| {
                string
                    .parse::<VersionReq>()
                    .map_err(|err| err.into())
                    .map(|version| (parsed_name, CrateDep::External(version)))
            }))
        }
        (name, CargoTomlDependency::Complex(cplx)) => {
            if cplx.git.is_some() {
                None
            } else if cplx.path.is_some() {
                cplx.path.map(|path| {
                    name.parse::<CrateName>()
                        .map(|parsed_name| (parsed_name, CrateDep::Internal(path)))
                })
            } else {
                cplx.version.as_deref().map(|version| {
                    let name = cplx.package.as_deref().unwrap_or(&name);
                    name.parse::<CrateName>().and_then(|parsed_name| {
                        version
                            .parse::<VersionReq>()
                            .map_err(|err| err.into())
                            .map(|version| (parsed_name, CrateDep::External(version)))
                    })
                })
            }
        }
    }
}

fn convert_package(cargo_package: Package) -> Option<Result<(CrateName, CrateDep), Error>> {
    let package_name = cargo_package.name.as_str().parse::<CrateName>().unwrap();
    let version_req = VersionReq::parse(&cargo_package.version.to_string()).unwrap();

    Some(Ok((package_name, CrateDep::External(version_req))))
}

pub fn parse_manifest_toml(input: &str) -> Result<CrateManifest, Error> {
    let cargo_toml = toml::de::from_str::<CargoToml>(input)?;

    let mut package_part = None;
    let mut workspace_part = None;

    if let Some(package) = cargo_toml.package {
        let crate_name = package.name.parse::<CrateName>()?;

        let dependencies = cargo_toml
            .dependencies
            .into_iter()
            .filter_map(convert_dependency)
            .collect::<Result<IndexMap<_, _>, _>>()?;
        let dev_dependencies = cargo_toml
            .dev_dependencies
            .into_iter()
            .filter_map(convert_dependency)
            .collect::<Result<IndexMap<_, _>, _>>()?;
        let build_dependencies = cargo_toml
            .build_dependencies
            .into_iter()
            .filter_map(convert_dependency)
            .collect::<Result<IndexMap<_, _>, _>>()?;

        let deps = CrateDeps {
            main: dependencies,
            dev: dev_dependencies,
            build: build_dependencies,
            unknown: IndexMap::new(),
        };

        package_part = Some((crate_name, deps));
    }

    if let Some(workspace) = cargo_toml.workspace {
        workspace_part = Some(workspace.members);
    }

    match (package_part, workspace_part) {
        (Some((name, deps)), None) => Ok(CrateManifest::Package(name, deps)),
        (None, Some(members)) => Ok(CrateManifest::Workspace { members }),
        (Some((name, deps)), Some(members)) => Ok(CrateManifest::Mixed {
            name,
            deps,
            members,
        }),
        (None, None) => Err(anyhow!("neither workspace nor package found in manifest")),
    }
}

pub fn parse_lock(input: &str) -> Result<CrateManifest, Error> {
    let lockfile = Lockfile::from_str(input).unwrap();
    let crate_name = CrateName::from_str("unused").unwrap();

    let unknown_dependencies = lockfile
        .packages
        .into_iter()
        .filter_map(convert_package)
        .collect::<Result<IndexMap<_, _>, _>>()?;

    let deps = CrateDeps {
        main: IndexMap::new(),
        dev: IndexMap::new(),
        build: IndexMap::new(),
        unknown: unknown_dependencies,
    };

    Ok(CrateManifest::Package(crate_name, deps))
}

#[cfg(test)]
mod tests {
    use crate::models::crates::CrateManifest;

    use super::*;

    #[test]
    fn parse_workspace_without_members_declaration() {
        let toml = r#"[package]
name = "symbolic"

[workspace]

[dependencies]
symbolic-common = { version = "2.0.6", path = "common" }
"#;

        let manifest = parse_manifest_toml(toml).unwrap();

        match manifest {
            CrateManifest::Mixed {
                name,
                deps,
                members,
            } => {
                assert_eq!(name.as_ref(), "symbolic");
                assert_eq!(deps.main.len(), 1);
                assert_eq!(deps.dev.len(), 0);
                assert_eq!(deps.build.len(), 0);
                assert_eq!(members.len(), 0);
            }
            _ => panic!("expected mixed manifest"),
        }
    }

    #[test]
    fn parse_manifest_with_renamed_deps() {
        let toml = r#"[package]
name = "symbolic"

[dependencies]
symbolic-common_crate = { version = "2.0.6", package = "symbolic-common" }
"#;

        let manifest = parse_manifest_toml(toml).unwrap();

        match manifest {
            CrateManifest::Package(name, deps) => {
                assert_eq!(name.as_ref(), "symbolic");
                assert_eq!(deps.main.len(), 1);
                assert_eq!(deps.dev.len(), 0);
                assert_eq!(deps.build.len(), 0);

                let name: CrateName = "symbolic-common".parse().unwrap();
                assert!(deps.main.get(&name).is_some());
            }
            _ => panic!("expected package manifest"),
        }
    }
}

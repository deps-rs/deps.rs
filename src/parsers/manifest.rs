use failure::Error;
use indexmap::IndexMap;
use relative_path::RelativePathBuf;
use semver::VersionReq;
use toml;

use crate::models::crates::{CrateName, CrateDep, CrateDeps, CrateManifest};

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlComplexDependency {
    git: Option<String>,
    path: Option<RelativePathBuf>,
    version: Option<String>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum CargoTomlDependency {
    Simple(String),
    Complex(CargoTomlComplexDependency)
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlPackage {
    name: String
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlWorkspace {
    #[serde(default)]
    members: Vec<RelativePathBuf>
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
    build_dependencies: IndexMap<String, CargoTomlDependency>
}

fn convert_dependency(cargo_dep: (String, CargoTomlDependency)) -> Option<Result<(CrateName, CrateDep), Error>> {
    match cargo_dep {
        (name, CargoTomlDependency::Simple(string)) => {
            Some(name.parse::<CrateName>().map_err(|err| err.into()).and_then(|parsed_name| {
                string.parse::<VersionReq>().map_err(|err| err.into())
                    .map(|version| (parsed_name, CrateDep::External(version)))
            }))
        }
        (name, CargoTomlDependency::Complex(cplx)) => {
            if cplx.git.is_some() {
                None
            } else if cplx.path.is_some() {
                cplx.path.map(|path| {
                    name.parse::<CrateName>().map_err(|err| err.into()).map(|parsed_name| {
                        (parsed_name, CrateDep::Internal(path))
                    })
                })
            } else {
                cplx.version.map(|string| {
                    name.parse::<CrateName>().map_err(|err| err.into()).and_then(|parsed_name| {
                        string.parse::<VersionReq>().map_err(|err| err.into())
                            .map(|version| (parsed_name, CrateDep::External(version)))
                    })
                })
            }
        }
    }
}

pub fn parse_manifest_toml(input: &str) -> Result<CrateManifest, Error> {
    let cargo_toml = toml::de::from_str::<CargoToml>(input)?;

    let mut package_part = None;
    let mut workspace_part = None;

    if let Some(package) =  cargo_toml.package {
        let crate_name = package.name.parse::<CrateName>()?;

        let dependencies = cargo_toml.dependencies
            .into_iter().filter_map(convert_dependency).collect::<Result<IndexMap<_, _>, _>>()?;
        let dev_dependencies = cargo_toml.dev_dependencies
            .into_iter().filter_map(convert_dependency).collect::<Result<IndexMap<_, _>, _>>()?;
        let build_dependencies = cargo_toml.build_dependencies
            .into_iter().filter_map(convert_dependency).collect::<Result<IndexMap<_, _>, _>>()?;

        let deps = CrateDeps {
            main: dependencies,
            dev: dev_dependencies,
            build: build_dependencies
        };

        package_part = Some((crate_name, deps));
    }

    if let Some(workspace) = cargo_toml.workspace {
        workspace_part = Some(workspace.members);
    }

    match (package_part, workspace_part) {
        (Some((name, deps)), None) =>
            Ok(CrateManifest::Package(name, deps)),
        (None, Some(members)) =>
            Ok(CrateManifest::Workspace { members }),
        (Some((name, deps)), Some(members)) =>
            Ok(CrateManifest::Mixed { name, deps, members }),
        (None, None) =>
            Err(format_err!("neither workspace nor package found in manifest"))
    }
}

#[cfg(test)]
mod tests {
    use models::crates::CrateManifest;
    use super::parse_manifest_toml;

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
            CrateManifest::Mixed { name, deps, members } => {
                assert_eq!(name.as_ref(), "symbolic");
                assert_eq!(deps.main.len(), 1);
                assert_eq!(deps.dev.len(), 0);
                assert_eq!(deps.build.len(), 0);
                assert_eq!(members.len(), 0);
            },
            _ => panic!("expected mixed manifest")
        }
    }
}

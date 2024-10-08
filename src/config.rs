pub mod file {
    /// Contains structures for parsing the nb.toml configuration file.

    use serde::Deserialize;
    use serde_inline_default::serde_inline_default;
    use std::collections::HashMap;

    /// Built-in default configuration.
    ///
    /// TODO: merge this file with user-supplied file?
    const DEFAULT_CONFIG: &str = include_str!("../default.toml");

    /// A nb.toml file.
    #[derive(Deserialize, Debug)]
    pub struct File {
        pub description: Option<String>,
        pub team: Option<String>,
        #[serde(default = "HashMap::new")]
        pub branch: HashMap<String, BranchRule>,
        #[serde(default = "Default::default")]
        pub sdk: Sdk,
        pub release: Release,
    }

    impl Default for File {
        fn default() -> Self {
            // The default config is compiled into the program, so
            // make sure to test default() to catch panics compile-time.
            toml::from_str(DEFAULT_CONFIG).unwrap()
        }
    }

    #[derive(Deserialize, Debug)]
    pub struct BranchRule {
        output: String,
        deploy: BranchDeployRule,
    }

    #[serde_inline_default]
    #[derive(Deserialize, Debug)]
    pub struct BranchDeployRule {
        pub environments: Vec<String>,
        //pub app_name_prefix: String,
        #[serde_inline_default(false)]
        pub parallel: bool,
    }

    #[derive(Deserialize, Debug)]
    pub struct Sdk {
        pub go: SdkGolang,
        pub rust: SdkRust,
    }

    impl Default for Sdk {
        fn default() -> Self {
            Self {
                go: Default::default(),
                rust: Default::default(),
            }
        }
    }

    #[derive(Deserialize, Debug)]
    pub struct SdkGolang {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
    }

    impl Default for SdkGolang {
        fn default() -> Self {
            Self {
                build_docker_image: "library/golang:1-alpine".to_string(),
                runtime_docker_image: "gcr.io/distroless/static-debian12:nonroot".to_string(),
            }
        }
    }

    #[derive(Deserialize, Debug)]
    pub struct SdkRust {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
    }

    impl Default for SdkRust {
        fn default() -> Self {
            Self {
                build_docker_image: "library/rust:1-alpine".to_string(),
                runtime_docker_image: "gcr.io/distroless/static-debian12:nonroot".to_string(),
            }
        }
    }

    #[derive(Deserialize, Debug)]
    pub struct Build {
        typ: String,
        sdk: String,
        docker: Docker,
    }

    #[derive(Deserialize, Debug)]
    pub struct Docker {
        image_name: String,
        image_tag: String,
        /*
        //auto_generate: bool,
        //output_files: Vec<String>,
        //user: PosixUser,
        //group: PosixGroup,
         */
    }

    #[derive(Deserialize, Debug)]
    pub struct Release {
        #[serde(rename = "type")]
        pub typ: ReleaseType,
        ghcr: ReleaseParams,
        gar: ReleaseParams,
    }

    impl Release {
        pub fn value(&self) -> ReleaseParams {
            match self.typ {
                ReleaseType::GAR => self.gar.clone(),
                ReleaseType::GHCR => self.ghcr.clone(),
            }
        }

        pub fn docker_name_builder(&self, config: super::docker::name::Config) -> Box<dyn ToString> {
            match self.typ {
                ReleaseType::GAR => Box::new(super::docker::name::GoogleArtifactRegistry(config)),
                ReleaseType::GHCR => Box::new(super::docker::name::GitHubContainerRegistry(config)),
            }
        }
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct ReleaseParams {
        pub registry: String,
    }

    #[derive(Deserialize, Debug, Eq, PartialEq)]
    pub enum ReleaseType {
        #[serde(rename = "gar")]
        /// Google Artifact Registry
        GAR,

        #[serde(rename = "ghcr")]
        /// GitHub Container Registry
        GHCR,
    }

    #[cfg(test)]
    pub mod test {
        use super::File;
        use crate::config::file::ReleaseType::GAR;

        #[test]
        pub fn load_default_configuration() {
            let cfg = File::default();
            assert_eq!(cfg.description, Some("Default configuration file".into()));
            assert_eq!(cfg.release.typ, GAR);
        }
    }
}

pub mod docker {
    /// Specifies how to format Docker image names.
    pub mod name {
        use std::fmt::Display;

        pub struct Config {
            pub registry: String,
            pub team: String,
            pub app: String,
            pub tag: String,
        }

        pub struct GoogleArtifactRegistry(pub Config);
        pub struct GitHubContainerRegistry(pub Config);

        impl Display for GoogleArtifactRegistry {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let registry = &self.0.registry;
                let team = &self.0.team;
                let app = &self.0.app;
                let tag = &self.0.tag;
                write!(f, "{}", format!("{registry}/{team}/{app}:{tag}"))
            }
        }

        impl Display for GitHubContainerRegistry {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let registry = &self.0.registry;
                let app = &self.0.app;
                let tag = &self.0.tag;
                write!(f, "{}", format!("{registry}/{app}:{tag}"))
            }
        }

        #[cfg(test)]
        pub mod tests {
            use super::*;

            fn configuration() -> Config {
                Config {
                    registry: "path/to/registry".to_string(),
                    team: "mynamespace".to_string(),
                    app: "myapplication".to_string(),
                    tag: "1-foo".to_string(),
                }
            }

            #[test]
            pub fn gar_release() {
                assert_eq!(GoogleArtifactRegistry(configuration()).to_string(), "path/to/registry/mynamespace/myapplication:1-foo".to_string());
            }

            #[test]
            pub fn ghcr_release() {
                assert_eq!(GoogleArtifactRegistry(configuration()).to_string(), "path/to/registry/myapplication:1-foo".to_string());
            }
        }
    }

    /// Specifies how to format Docker image tags.
    pub mod tag {
        use thiserror::Error;
        use Error::*;

        #[derive(Debug, Error)]
        pub enum Error {
            #[error("failed to execute Git: {0}")]
            FailedExecute(#[from] std::io::Error),

            #[error("failed to parse Git short SHA: {0}")]
            ParseGitShortSha(#[from] std::string::FromUtf8Error),
        }

        /// Generate a Docker tag with the current timestamp and the currently checked out Git short SHA sum.
        ///
        /// Git-related values will be generated by the currently installed `git` executable.
        /// Returns an error if `git` is not installed, or if short sha parsing failed.
        ///
        /// If working tree is dirty, tag will be suffixed with `-dirty`.
        ///
        /// Example output: `20241008.152558.abcdef`
        pub fn generate(filesystem_path: &str) -> Result<String, Error> {
            let now = chrono::Local::now();
            let datetime = now.format("%Y%m%d.%H%M%S").to_string();

            let git_tree_dirty = std::process::Command::new("git")
                .arg("ls-files")
                .arg("--exclude-standard")
                .arg("--others")
                .current_dir(filesystem_path)
                .output()
                .map(|output| output.stdout.len() > 0)
                .map_err(FailedExecute)?;

            let git_short_sha = std::process::Command::new("git")
                .arg("rev-parse")
                .arg("--short").arg("HEAD")
                .current_dir(filesystem_path)
                .output()
                .map(|output| String::from_utf8(output.stdout))
                .map_err(FailedExecute)?
                .map_err(ParseGitShortSha)
                .map(|short_sha| short_sha.trim().to_string())
                ?;

            Ok(match git_tree_dirty {
                true => format!("{datetime}.{git_short_sha}-dirty"),
                false => format!("{datetime}.{git_short_sha}"),
            })
        }
    }
}

pub mod toml_merge {
    // from https://github.com/mrnerdhair/toml-merge/blob/c44eee7c7fa52e98b34c20c88d19979adcafcf1b/src/main.rs
    fn merge(merged: &mut toml::Value, value: &toml::Value) {
        match value {
            toml::Value::String(_) |
            toml::Value::Integer(_) |
            toml::Value::Float(_) |
            toml::Value::Boolean(_) |
            toml::Value::Datetime(_) => *merged = value.clone(),
            toml::Value::Array(x) => {
                match merged {
                    toml::Value::Array(merged) => {
                        for (k, v) in x.iter().enumerate() {
                            match merged.get_mut(k) {
                                Some(x) => merge(x, v),
                                None => {
                                    let _ = merged.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                    _ => *merged = value.clone(),
                }
            }
            toml::Value::Table(x) => {
                match merged {
                    toml::Value::Table(merged) => {
                        for (k, v) in x.iter() {
                            match merged.get_mut(k) {
                                Some(x) => merge(x, v),
                                None => {
                                    let _ = merged.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                    _ => *merged = value.clone(),
                }
            }
        }
    }

    /// Merge one or more TOML files into one.
    /// Returns a String with the merged TOMLs.
    ///
    /// FIXME: this function doesn't return valid TOML yet
    pub fn merge_files(file_contents: &[&str]) -> Result<String, toml::de::Error> {
        let mut merged: toml::Value = toml::Value::Table(toml::value::Table::new());
        for toml_data in file_contents.iter() {
            let value: toml::value::Table = toml::from_str(toml_data)?;
            merge(&mut merged, &toml::Value::Table(value));
        }
        Ok(toml::to_string_pretty(&merged).unwrap())
    }
}

pub mod file {
    /// Contains structures for parsing the nb.toml configuration file.

    use serde::{Deserialize, Serialize};
    use serde_inline_default::serde_inline_default;
    use std::collections::HashMap;
    use thiserror::Error;
    use crate::config::file::Error::{ParseConfig, ReadConfig, Serialization};

    /// Built-in default configuration.
    pub const DEFAULT_CONFIG: &str = include_str!("../default.toml");

    #[derive(Debug, Error)]
    pub enum Error {
        #[error(r#"read "{filename}": {err}"#)]
        ReadConfig {
            err: std::io::Error,
            filename: String,
        },

        #[error(r#"parse "{filename}": {err}"#)]
        ParseConfig {
            err: toml::de::Error,
            filename: String,
        },

        #[error("{0}")]
        Serialization(toml::de::Error),
    }

    /// A nb.toml file.
    #[derive(Serialize, Deserialize, Debug)]
    pub struct File {
        pub description: Option<String>,
        pub team: Option<String>,
        #[serde(default = "HashMap::new")]
        pub branch: HashMap<String, BranchRule>,
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

    impl File {
        pub fn default_with_user_config_file(filename: &str) -> Result<Self, Error> {
            let config_string = std::fs::read_to_string(filename)
                .map_err(|err| { ReadConfig { err, filename: filename.to_string() } })?;

            if let Err(err) = toml::from_str::<File>(&config_string) {
                return Err(ParseConfig { err, filename: filename.to_string() });
            }

            let merged_config_string = super::toml_merge::merge_files(&[
                DEFAULT_CONFIG,
                &config_string,
            ])
                .map_err(Serialization)?;

            Ok(
                toml::from_str::<File>(&merged_config_string)
                    .map_err(|err| ParseConfig { err, filename: filename.to_string() })?
            )
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct BranchRule {
        output: String,
        deploy: BranchDeployRule,
    }

    #[serde_inline_default]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct BranchDeployRule {
        pub environments: Vec<String>,
        //pub app_name_prefix: String,
        #[serde_inline_default(false)]
        pub parallel: bool,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Sdk {
        pub go: SdkGolang,
        pub rust: SdkRust,
        pub gradle: SdkGradle,
        pub maven: SdkMaven,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SdkGolang {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SdkRust {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SdkGradle {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
        pub settings_file: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SdkMaven {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Build {
        typ: String,
        sdk: String,
        docker: Docker,
    }

    #[derive(Serialize, Deserialize, Debug)]
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

    #[derive(Serialize, Deserialize, Debug)]
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

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReleaseParams {
        pub registry: String,
    }

    #[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
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
                assert_eq!(GitHubContainerRegistry(configuration()).to_string(), "path/to/registry/myapplication:1-foo".to_string());
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
        /// Example output: `20241008.152558.abcdef` or `20241008.152558.abcdef-dirty`
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

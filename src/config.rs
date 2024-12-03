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
    pub fn merge_files(file_contents: &[&str]) -> Result<String, toml::de::Error> {
        let mut merged: toml::Value = toml::Value::Table(toml::value::Table::new());
        for toml_data in file_contents.iter() {
            let value: toml::value::Table = toml::from_str(toml_data)?;
            merge(&mut merged, &toml::Value::Table(value));
        }
        Ok(toml::to_string_pretty(&merged).unwrap())
    }
}

pub mod runtime {
    use serde::{Deserialize, Serialize};
    use serde_inline_default::serde_inline_default;
    use thiserror::Error;
    use crate::docker;
    use crate::nais_yaml::NaisYaml;

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

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Sdk {
        pub go: SdkGolang,
        pub rust: SdkRust,
        pub gradle: SdkGradle,
        pub maven: SdkMaven,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SdkGolang {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SdkRust {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SdkGradle {
        pub build_docker_image: String,
        pub runtime_docker_image: String,
        pub settings_file: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
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

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ReleaseParams {
        pub registry: String,
    }

    #[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
    pub enum ReleaseType {
        #[serde(rename = "gar")]
        /// Google Artifact Registry
        GAR,

        #[serde(rename = "ghcr")]
        /// GitHub Container Registry
        GHCR,
    }

    pub struct Release {
        pub typ: ReleaseType,
        pub params: ReleaseParams,
    }

    impl Release {
        pub fn docker_name_builder(&self, config: docker::name::Config) -> Box<dyn ToString> {
            match self.typ {
                ReleaseType::GAR => Box::new(docker::name::GoogleArtifactRegistry(config)),
                ReleaseType::GHCR => Box::new(docker::name::GitHubContainerRegistry(config)),
            }
        }
    }

    pub struct Config {
        pub app: String,
        pub team: String,
        pub release: Release,
    }

    #[derive(Debug, Clone, Error)]
    pub enum Error {
        #[error("missing configuration")]
        MissingConfig,
    }

    impl Config {
        /// Extract essential configuration from many sources, including build.toml and nais.yaml.
        pub fn new(
            cfg: &super::file::File,
            nais_yaml: NaisYaml,
        ) -> Result<Config, Error> {
            let release = cfg.release.clone().ok_or(Error::MissingConfig)?;
            let release_params = release.params_for_type();
            Ok(Config {
                app: nais_yaml.app,
                team: cfg.team.clone().unwrap_or(nais_yaml.team),
                release: Release {
                    typ: release.typ,
                    params: release_params,
                },
            })
        }
    }

}

pub mod file {
    /// Contains structures for parsing the nb.toml configuration file.

    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use thiserror::Error;
    use crate::config::file::Error::{ParseConfig, ReadConfig, Serialization};
    use crate::config::runtime::{BranchRule, ReleaseParams, ReleaseType, Sdk};

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
        pub sdk: Option<Sdk>,
        pub release: Option<Release>,
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

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Release {
        #[serde(rename = "type")]
        pub typ: ReleaseType,
        ghcr: ReleaseParams,
        gar: ReleaseParams,
    }

    impl Release {
        pub fn params_for_type(&self) -> ReleaseParams {
            match self.typ {
                ReleaseType::GAR => self.gar.clone(),
                ReleaseType::GHCR => self.ghcr.clone(),
            }
        }
    }

    #[cfg(test)]
    pub mod test {
        use super::File;
        use crate::config::file::ReleaseType::GAR;

        #[test]
        pub fn load_default_configuration() {
            let cfg = File::default();
            let release = cfg.release.unwrap();
            assert_eq!(cfg.description, Some("Default configuration file".into()));
            assert_eq!(release.typ, GAR);
            assert!(release.gar.registry.len() > 0);
        }
    }
}

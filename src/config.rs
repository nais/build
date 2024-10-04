use serde::Deserialize;
use serde_inline_default::serde_inline_default;
use std::collections::HashMap;
//use crate::oci::{PosixGroup, PosixUser};

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

#[cfg(test)]
pub mod test {
    #[test]
    pub fn load_default_configuration() {
        let cfg = super::File::default();
        assert_eq!(cfg.description, Some("Default configuration file".into()))
    }
}

use std::collections::HashMap;
use crate::oci::{PosixGroup, PosixUser};

/// A nb.toml file.
pub struct ConfigFile {
    pub description: String,
    pub team: String,
    pub branch: HashMap<String, BranchRule>,
    pub sdk: Sdk,
}

pub struct BranchRule {
    output: String,
    deploy: BranchDeployRule,
}

pub struct BranchDeployRule {
    pub environments: Vec<String>,
}

pub struct Sdk {
    rust: SdkRust,
}

pub struct SdkRust {
    build_docker_image: String,
    runtime_docker_image: String,
}

pub struct Build {
    typ: String,
    sdk: String,
    docker: Docker,
}

pub struct Docker {
    tag: String,
    auto_generate: bool,
    output_files: Vec<String>,
    user: PosixUser,
    group: PosixGroup,
}

#![allow(dead_code)]

mod config;
mod docker;
mod nais_yaml;
mod oci;
mod pipeline;
mod sdk;
mod deploy;
mod google;
mod git;

use std::fmt::{Display, Formatter};

struct BuildInput {
    nais_yaml: Option<String>,
    source_directory: Option<String>,
}

enum DeployConfig {
    Kubernetes(KubernetesDeployConfig),
    CDN(CDNDeployConfig),
    GithubBinaryRelease(GithubBinaryReleaseConfig),
}

/// nb = NAIS build
/// ---------------
/// nb config create
/// nb build
/// nb deploy

const EXAMPLE: &str = r#"
# Empty config will result in an auto-detected build.
[[build]]
sdk = rust
sdk_version = nightly
"#;

struct SignatureAndAttestation;

struct FileTree(String);

struct Binary(String);

enum BuildOutcome {
    /// Build an application inside Docker and package it as an image.
    /// SLSA signatures and attestation.
    SignedAndAttestedDockerImage,

    /// A directory that is an artifact of itself, for example a directory of static web files.
    FileTree,

    /// Executable file.
    Binaries,
}

struct UploadArtifact {
    docker_image: DockerImage,
    sign_attest: SignatureAndAttestation,
}

struct DockerImage(String);

struct PushDockerImageParams {
    server: String,
    credentials: String,
    image_name: String,
}


struct NaisDeployInputParams {
    artifact: UploadArtifact,
    kubernetes_deploy_config: KubernetesDeployConfig,
}

enum BranchDeploySuffix {
    /// Deploy application with its original name.
    NoSuffix,

    /// Deploy application with name 'myapplication-<branch>'
    BranchNameSuffix,

    /// Deploy application with name 'myapplication-<string>'
    ManualSuffix(String),
}

struct BranchDeployOptions {
    branch: String,
    suffix: BranchDeploySuffix,
}

/// Unique per customer, do we want an enum or a String?
struct Cluster(String);

struct ClusterOptions {
    /// Which Kubernetes cluster to deploy to.
    cluster: Cluster,

    /// Where to deploy the different branches.
    /// e.g. 'main' deploys with suffix NoSuffix, and
    /// 'q1' deploys with suffix BranchNameSuffix, and
    /// '*' deploys with suffix ManualSuffix.
    branch_deploys: BranchDeployOptions,

    /// These environments are triggered _after_ the current environment has succeeded.
    dependants: Vec<ClusterOptions>,
}

/// How do we deploy?

struct DeployServer {
    tenant: String,
}

impl Display for DeployServer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("https://deploy.{}.cloud.nais.io", self.tenant))
    }
}

/// Stuff needed to create a NAIS deploy on Kubernetes
struct KubernetesDeployConfig {
    nais_yaml_template: String,
    cluster: String,
    namespace: String,
    deploy_server: String, // https://deploy.<TENANT>.cloud.nais.io on gRPC
}

struct CDNDeployConfig {
    team: String,
    subdirectory: String,
}

struct GithubBinaryReleaseConfig {
    repository: String,
}

#![allow(dead_code)]

mod pipeline;

use std::collections::HashMap;
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

enum Version {
    Latest,
    Major(String),
    Exact(String),
}

enum SDK {
    Go(Version),
    Rust(Version),
    Java(Version),
}

struct PosixUser {
    id: usize,
    name: String,
}

struct PosixGroup {
    id: usize,
    name: String,
}

struct DockerImage(String);

struct PushDockerImageParams {
    server: String,
    credentials: String,
    image_name: String,
}

struct BuildDockerImageParams {
    /// Docker image for building the application.
    builder_image: DockerImage,

    /// Run the following build script inside the build container.
    build_script: String,

    /// Which image to use as output base image.
    base_image: DockerImage,

    /// How to name the output Docker image.
    output_image: DockerImage,

    /// Which user to set up as the application owner inside the image.
    /// The application will be run as this user.
    user: PosixUser,

    /// Which group to set up as the application owner inside the image.
    /// The application will be run as this group.
    group: PosixGroup,

    /// Files to copy into the build container.
    input_files: Vec<String>,

    /// Files to copy from the build container to the application image,
    /// once the build script has been run to completion.
    output_files: HashMap<String, String>,

    /// Auto-expose these ports
    ports: Vec<u16>,

    /// Default values for environment variables.
    env_vars: HashMap<String, String>,
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

struct ConfigFile {

}
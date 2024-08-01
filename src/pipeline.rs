/// Docker image name and tag combination.
type ImageID = String;

/// Templatable Kubernetes YAML file
struct ResourceYaml;

struct File;

struct Directory;

struct NaisDeployMetadata {
    /// build.nais.io/client-version
    nais_build_version: String,

    repository: String,

    // auto-detected by nais deploy, maybe enough to keep them in the templating step
    team: String,

    // these parameters are already loaded by nais deploy using environment variables
    // available in the Github environment.
    // is it necessary to include them here?
    git_sha: String,
    correlation_id: String,
    actor: String,
}

struct NaisDeployUnit {
    resources: Vec<ResourceYaml>,
    metadata: NaisDeployMetadata,
    // destination is cluster+tenant, but is not a part of the "artifact" itself
}

/// Various things that can be built.
/// Builds produce artifacts, which can then be published or deployed.
enum Artifact {
    /// SLSA signatures and attestation for a Docker image.
    SLSA(ImageID),

    /// Docker image produced on the local machine.
    DockerImage(ImageID),

    /// A nais deploy artifact is the collection of data and parameters
    /// required to deploy an image as a container on the NAIS platform.
    NaisDeploy(NaisDeployUnit),
    Binary(File),
    Directory(Directory),
}

/// Various ways to publish an artifact.
/// Publishing is defined as storing the artifact's data and metadata at a well-known location.
enum Publish {
    /// Upload a Docker image and its associated SLSA data to Google Artifact Registry.
    ArtifactRegistry,

    /// Create a GitHub release and add the files to it.
    /// https://docs.github.com/en/rest/releases/releases?apiVersion=2022-11-28#create-a-release
    GitHubRelease(Vec<File>),
}

/// Various ways to deploy an artifact.
/// Deploying things means that the runtime environment is affected.
/// Thus, a deploy is visible to end users.
enum Deploy {
    /// Deploy a containerized app.
    /// These deploys need a combination of a Docker image and an Application spec.
    /// - detect image name
    /// - detect customer (NAV)
    /// - detect environment (prod-gcp)
    /// - detect namespace (team)
    /// - branched deployments for PR's
    NaisDeploy(NaisDeployUnit),

    /// Deploy a directory to the team's CDN bucket.
    /// - detect source
    /// - detect destination
    /// - upload to bucket
    CDNDeploy(Directory),
}


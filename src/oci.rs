use std::collections::HashMap;

const NAIS_DEFAULT_UID_GID: usize = 1069;

/// UID/username combination inside a POSIX environment.
pub struct PosixUser {
    pub uid: usize,
    pub name: String,
}

impl Default for PosixUser {
    fn default() -> Self {
        Self {
            uid: NAIS_DEFAULT_UID_GID,
            name: "nobody".into(),
        }
    }
}

/// GID/groupname combination inside a POSIX environment.
pub struct PosixGroup {
    pub gid: usize,
    pub name: String,
}

impl Default for PosixGroup {
    fn default() -> Self {
        Self {
            gid: NAIS_DEFAULT_UID_GID,
            name: "nogroup".into(),
        }
    }
}

/// Reference to a docker image
#[derive(Debug, PartialEq)]
pub struct DockerImage(pub String);

pub struct DockerBuildParams {
    /// Docker image for building the application.
    pub builder_image: DockerImage,

    /// Which image to use as output base image.
    pub base_image: DockerImage,

    /// How to name the output Docker image.
    pub output_image: DockerImage,

    /// Run the following build script inside the build container.
    pub build_script: String,

    /// Which user to set up as the application owner inside the image.
    /// The application will be run as this user.
    pub user: PosixUser,

    /// Which group to set up as the application owner inside the image.
    /// The application will be run as this group.
    pub group: PosixGroup,

    /// Files to copy into the build container.
    pub input_files: Vec<String>,

    /// Files to copy from the build container to the application image,
    /// once the build script has been run to completion.
    pub output_files: HashMap<String, String>,

    /// Auto-expose these ports
    //pub ports: Vec<u16>,

    /// Default values for environment variables.
    pub env_vars: HashMap<String, String>,
}

pub trait DockerBuilder {
    fn build_params(&self, target: &str) -> DockerBuildParams;
}
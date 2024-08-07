use crate::DockerImage;
use crate::oci::DockerBuildParams;

pub enum Error {}

pub fn build(_params: DockerBuildParams) -> Result<DockerImage, Error> {
    todo!()
}

pub mod rust {
    use crate::oci::{DockerBuilder, DockerBuildParams, DockerImage, PosixGroup, PosixUser};
    use crate::sdk::{Version};

    /// A builder that can build Rust programs.
    pub struct Rust {
        version: Version,
    }

    impl Rust {
        fn new(version: Version) -> Self {
            Self {
                version,
            }
        }

        pub fn docker_builder_image(&self) -> DockerImage {
            DockerImage(format!("library/rust:{}",
                                match &self.version {
                                    Version::Latest => "alpine".into(),
                                    Version::Major(major) => format!("{}-alpine", major),
                                    Version::Exact(exact) => format!("{}-alpine", exact),
                                }))
        }
    }

    impl Default for Rust {
        fn default() -> Self {
            Self {
                version: Version::Latest,
            }
        }
    }

    impl DockerBuilder for Rust {
        fn build_params(&self, target: &str) -> DockerBuildParams {
            DockerBuildParams {
                builder_image: self.docker_builder_image(),
                base_image: DockerImage("library/alpine:3".into()),
                output_image: DockerImage("output/image:tag".into()),
                build_script: format!("cargo build --release --bin {}", target),
                user: PosixUser::default(),
                group: PosixGroup::default(),
                input_files: vec![],
                output_files: Default::default(),
                env_vars: Default::default(),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::oci::DockerImage;
        use super::Rust;
        use crate::sdk::{Version};

        #[test]
        fn rust_docker_images() {
            assert_eq!(
                Rust::new(Version::Latest).docker_builder_image(),
                DockerImage("library/rust:alpine".to_string())
            );
            assert_eq!(
                Rust::new(Version::Major(1)).docker_builder_image(),
                DockerImage("library/rust:1-alpine".to_string())
            );
            assert_eq!(
                Rust::new(Version::Exact("1.80".into())).docker_builder_image(),
                DockerImage("library/rust:1.80-alpine".to_string())
            );
        }
    }
}
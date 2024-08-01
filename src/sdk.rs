pub enum Version {
    Latest,
    Major(usize),
    Exact(String),
}

pub enum SDK {
    Go,
    Rust,
    Java,
}

pub struct Rust;

pub struct Go;

pub struct Java;

pub struct SdkVersion {
    sdk: SDK,
    version: Version,
}

impl SdkVersion {
    pub fn new(sdk: SDK, version: Version) -> SdkVersion {
        Self { sdk, version }
    }
}

pub trait Sdk {
    fn docker_builder_image(_: Version) -> String;
    fn docker_base_image() -> String;
}

impl Rust {
    fn builder_image_version (version:Version) -> String {
        match version {
            Version::Latest => "alpine".into(),
            Version::Major(major) => format!("{}-alpine", major),
            Version::Exact(exact) => format!("{}-alpine", exact),
        }
    }
}

impl Sdk for Rust {
    fn docker_builder_image(version: Version) -> String {
        format!("{}:{}", "library/rust", Self::builder_image_version(version))
    }

    fn docker_base_image() -> String {
        "library/alpine:3".into()
    }
}

#[cfg(test)]
mod tests {
    use crate::sdk::{Rust, Sdk, Version};

    #[test]
    fn rust_docker_images() {
        assert_eq!(
            Rust::docker_builder_image(Version::Latest),
            "library/rust:alpine".to_string()
        );
        assert_eq!(
            Rust::docker_builder_image(Version::Major(1)),
            "library/rust:1-alpine".to_string()
        );
        assert_eq!(
            Rust::docker_builder_image(Version::Exact("1.80".into())),
            "library/rust:1.80-alpine".to_string()
        );
    }
}
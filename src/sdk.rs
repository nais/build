use crate::oci::{DockerBuilder, DockerBuildParams};

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

pub struct Go;

impl DockerBuilder for Go {
    fn build_params(&self, target: &str) -> DockerBuildParams {
        todo!()
    }
}
use std::io::Write;
use std::process::{ExitStatus, Stdio};
use log::debug;
use thiserror::Error;
use crate::docker::Error::IOError;
use crate::sdk;
use crate::sdk::DockerFileBuilder;

#[derive(Error, Debug)]
pub enum Error {
    #[error("docker build failed with exit code {0}")]
    Build(ExitStatus),

    #[error("dockerfile generation failed: {0}")]
    Generate(sdk::Error),

    #[error("docker login failed with exit code {0}")]
    Login(ExitStatus),

    #[error("docker logout failed with exit code {0}")]
    Logout(ExitStatus),

    #[error("docker push failed with exit code {0}")]
    Push(ExitStatus),

    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

pub fn build(docker_file_builder: Box<dyn DockerFileBuilder>, tag: &str) -> Result<(), Error> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(docker_file_builder.dockerfile().map_err(Error::Generate)?.as_bytes())?;

    std::process::Command::new("docker")
        .arg("build")
        .arg("--file")
        .arg(file.path())
        .arg("--tag")
        .arg(tag)
        .arg(docker_file_builder.filesystem_path())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map(|exit_status| {
            if exit_status.success() {
                Ok(())
            } else {
                Err(Error::Build(exit_status))
            }
        })?
}



pub fn login(registry: String, token: String) -> Result<(), Error> {
    debug!("Logging in to Docker registry {}", registry);
    let mut child = std::process::Command::new("docker")
        .arg("login")
        .arg(registry)
        .arg("--username")
        .arg("oauth2accesstoken") // TODO: this only works for GAR
        .arg("--password-stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn().map_err(IOError)?;

    child.stdin.as_mut().unwrap().write_all(token.as_bytes())?;
    let status = child.wait_with_output()?.status;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Login(status))
    }
}

pub fn logout(registry: String) -> Result<(), Error> {
    std::process::Command::new("docker")
        .arg("logout")
        .arg(registry)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map(|exit_status| {
            if exit_status.success() {
                Ok(())
            } else {
                Err(Error::Logout(exit_status))
            }
        })?
}

pub fn push(image_name: String) -> Result<(), Error> {
    debug!("Pushing image {}", image_name);
    std::process::Command::new("docker")
        .arg("push")
        .arg(image_name)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map(|exit_status| {
            if exit_status.success() {
                Ok(())
            } else {
                Err(Error::Push(exit_status))
            }
        })?
}

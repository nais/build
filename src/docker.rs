use std::io::Write;
use std::process::{ExitStatus, Stdio};
use log::debug;
use thiserror::Error;
use crate::docker::Error::IOError;
use crate::sdk;
use crate::sdk::SDK;

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

/// Specifies how to format Docker image names.
pub mod name {
    use std::fmt::Display;

    pub struct Config {
        pub registry: String,
        pub team: String,
        pub app: String,
        pub tag: String,
    }

    pub struct GoogleArtifactRegistry(pub Config);
    pub struct GitHubContainerRegistry(pub Config);

    impl Display for GoogleArtifactRegistry {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let registry = &self.0.registry;
            let team = &self.0.team;
            let app = &self.0.app;
            let tag = &self.0.tag;
            write!(f, "{}", format!("{registry}/{team}/{app}:{tag}"))
        }
    }

    impl Display for GitHubContainerRegistry {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let registry = &self.0.registry;
            let app = &self.0.app;
            let tag = &self.0.tag;
            write!(f, "{}", format!("{registry}/{app}:{tag}"))
        }
    }

    #[cfg(test)]
    pub mod tests {
        use super::*;

        fn configuration() -> Config {
            Config {
                registry: "path/to/registry".to_string(),
                team: "mynamespace".to_string(),
                app: "myapplication".to_string(),
                tag: "1-foo".to_string(),
            }
        }

        #[test]
        pub fn gar_release() {
            assert_eq!(GoogleArtifactRegistry(configuration()).to_string(), "path/to/registry/mynamespace/myapplication:1-foo".to_string());
        }

        #[test]
        pub fn ghcr_release() {
            assert_eq!(GitHubContainerRegistry(configuration()).to_string(), "path/to/registry/myapplication:1-foo".to_string());
        }
    }
}

/// Specifies how to format Docker image tags.
pub mod tag {
    use thiserror::Error;
    use Error::*;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error("failed to execute Git: {0}")]
        FailedExecute(#[from] std::io::Error),

        #[error("failed to parse Git short SHA: {0}")]
        ParseGitShortSha(#[from] std::string::FromUtf8Error),
    }

    /// Generate a Docker tag with the current timestamp and the currently checked out Git short SHA sum.
    ///
    /// Git-related values will be generated by the currently installed `git` executable.
    /// Returns an error if `git` is not installed, or if short sha parsing failed.
    ///
    /// If working tree is dirty, tag will be suffixed with `-dirty`.
    ///
    /// Example output: `20241008.152558.abcdef` or `20241008.152558.abcdef-dirty`
    pub fn generate(filesystem_path: &str) -> Result<String, Error> {
        let now = chrono::Local::now();
        let datetime = now.format("%Y%m%d.%H%M%S").to_string();

        let git_tree_dirty = std::process::Command::new("git")
            .arg("ls-files")
            .arg("--exclude-standard")
            .arg("--others")
            .current_dir(filesystem_path)
            .output()
            .map(|output| output.stdout.len() > 0)
            .map_err(FailedExecute)?;

        let git_short_sha = std::process::Command::new("git")
            .arg("rev-parse")
            .arg("--short").arg("HEAD")
            .current_dir(filesystem_path)
            .output()
            .map(|output| String::from_utf8(output.stdout))
            .map_err(FailedExecute)?
            .map_err(ParseGitShortSha)
            .map(|short_sha| short_sha.trim().to_string())
            ?;

        Ok(match git_tree_dirty {
            true => format!("{datetime}.{git_short_sha}-dirty"),
            false => format!("{datetime}.{git_short_sha}"),
        })
    }
}

pub fn build(docker_file_builder: &Box<dyn SDK>, tag: &str) -> Result<(), Error> {
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

pub fn login(registry: &str, token: &str) -> Result<(), Error> {
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

pub fn logout(registry: &str) -> Result<(), Error> {
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

pub fn push(image_name: &str) -> Result<(), Error> {
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

/// NAIS Build

use crate::Error::*;
use clap::{Parser, Subcommand};
use std::io::Write;
use std::process::{ExitStatus, Stdio};
use thiserror::Error;

#[allow(dead_code)]
mod config;

/// NAISly build, test, lint, check and deploy your application.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Root of the source code tree.
    #[arg(short, long, default_value = ".")]
    source_directory: String,

    /// Path to the NAIS build configuration file.
    // ... TODO: or nais.toml?
    #[arg(short, long)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Detect build parameters and print your Dockerfile.
    Dockerfile,
    /// Build your project into a Dockerfile.
    Build,
}

#[derive(Error, Debug)]
pub enum DetectBuildTargetError {
    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("target name is empty")]
    EmptyFilename,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("no compatible SDKs for this source directory")]
    SDKNotDetected,

    #[error("detect build target: {0}")]
    DetectBuildTargetError(#[from] DetectBuildTargetError),

    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("configuration file syntax error: {0}")]
    ConfigDeserialize(#[from] toml::de::Error),

    #[error("docker tag could not be generated: {0}")]
    DockerTag(#[from] config::docker::tag::Error),

    #[error("docker build failed with exit code {0}")]
    DockerBuild(ExitStatus),
}

/// Read configuration file from disk.
///
/// If a configuration file name is not set explicitly, this function will
/// detect whether a config file with the default file name exists on disk.
/// If it does, it is used implicitly. If not, we ignore any read errors.
fn read_config(args: &Cli) -> Result<config::file::File, Error> {
    const DEFAULT_CONFIG_FILE: &str = "nb.toml";
    let config_file = match &args.config {
        None => {
            if std::fs::metadata(DEFAULT_CONFIG_FILE)
                .and_then(|metadata| Ok(metadata.is_file()))
                .unwrap_or(false)
            {
                Some(DEFAULT_CONFIG_FILE.into())
            } else {
                None
            }
        }
        Some(c) => Some(c.clone()),
    };

    Ok(if let Some(config_file) = config_file {
        let config_string = std::fs::read_to_string(&config_file)?;
        toml::from_str::<config::file::File>(&config_string)?
    } else {
        config::file::File::default()
    })
}

fn main() -> Result<(), Error> {
    let args = Cli::parse();
    let cfg = read_config(&args)?;

    let docker_image_name = cfg.release
        .docker_name_builder(config::docker::name::Config {
            registry: cfg.release.value().registry,
            tag: config::docker::tag::generate(&args.source_directory)?,
            // from nais.yml?
            team: "myteam".to_string(),
            app: "myapp".to_string(),
        })
        .to_string();

    match args.command {
        Commands::Dockerfile => {
            let sdk = init_sdk(&args.source_directory, &cfg)?;
            println!("{}\n\n", sdk.dockerfile()?);
            eprintln!("Will be built as: {}", docker_image_name);
            Ok(())
        }
        Commands::Build => {
            let sdk = init_sdk(&args.source_directory, &cfg)?;
            // Self {
            //                 filesystem_path: filesystem_path.into(),
            //                 docker_image_name_tagged: ,
            //                 start_hook: None,
            //                 end_hook: None,
            //             }

            build(sdk, &docker_image_name)?;
            Ok(())
        }
    }
}

/// SDK is anything that can produce artifacts
trait DockerFileBuilder {
    fn builder_docker_image(&self) -> String;
    fn runtime_docker_image(&self) -> String;
    fn detect_build_targets(&self) -> Result<Vec<String>, DetectBuildTargetError>;
    fn dockerfile(&self) -> Result<String, Error>;
    fn filesystem_path(&self) -> String;
    //fn docker_image_name_tagged(&self) -> String;
}


fn build(docker_file_builder: Box<dyn DockerFileBuilder>, tag: &str) -> Result<(), Error> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(docker_file_builder.dockerfile()?.as_bytes())?;

    std::process::Command::new("docker")
        .arg("build")
        .arg("--file").arg(file.path())
        .arg("--tag").arg(tag)
        .arg(docker_file_builder.filesystem_path())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map(|exit_status| {
            if exit_status.success() {
                Ok(())
            } else {
                Err(DockerBuild(exit_status))
            }
        })?
}
fn init_sdk(filesystem_path: &str, cfg: &config::file::File) -> Result<Box<dyn DockerFileBuilder>, Error> {
    match golang::new(golang::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: cfg.sdk.go.build_docker_image.clone(),
        docker_runtime_image: cfg.sdk.go.runtime_docker_image.clone(),
        start_hook: None,
        end_hook: None,
    }) {
        Ok(Some(sdk)) => {
            return Ok(Box::new(sdk));
        }
        Ok(None) => {}
        Err(err) => return Err(err),
    }

    // try next sdk

    Err(SDKNotDetected)
}

/// Build Go projects.
pub mod golang {
    use crate::{DetectBuildTargetError, DockerFileBuilder, Error};
    use crate::DetectBuildTargetError::EmptyFilename;

    pub struct Golang(Config);

    pub struct Config {
        pub filesystem_path: String,
        pub docker_builder_image: String,
        pub docker_runtime_image: String,

        #[allow(dead_code)]
        pub start_hook: Option<String>,
        #[allow(dead_code)]
        pub end_hook: Option<String>,
    }

    pub fn new(cfg: Config) -> Result<Option<Golang>, Error> {
        let Ok(file_stat) = std::fs::metadata(cfg.filesystem_path.to_owned() + "/go.mod") else {
            return Ok(None);
        };
        if !file_stat.is_file() {
            return Ok(None);
        }


        Ok(Some(Golang(cfg)))
    }

    impl DockerFileBuilder for Golang {
        fn builder_docker_image(&self) -> String {
            self.0.docker_builder_image.clone()
        }

        fn runtime_docker_image(&self) -> String {
            self.0.docker_runtime_image.clone()
        }

        /// Return a list of binaries that can be built.
        fn detect_build_targets(&self) -> Result<Vec<String>, DetectBuildTargetError> {
            std::fs::read_dir(self.0.filesystem_path.to_owned() + "/cmd")?
                .map(|dir_entry| {
                    Ok(dir_entry?
                        .file_name()
                        .to_str()
                        .ok_or(EmptyFilename)?
                        .to_string())
                })
                .collect()
        }

        fn dockerfile(&self) -> Result<String, Error> {
            let targets = self.detect_build_targets()?;
            let builder_image = &self.builder_docker_image();
            let runtime_image = &self.runtime_docker_image();
            let binary_build_commands: String = targets
                .iter()
                .map(|item| {
                    format!(
                        "RUN go build -a -installsuffix cgo -o /build/{} ./cmd/{}",
                        item, item
                    )
                })
                .fold(String::new(), |acc, item| acc + "\n" + &item)
                .trim()
                .to_string();
            let binary_copy_commands: String = targets
                .iter()
                .map(|item| format!("COPY --from=builder /build/{} /app/{}", item, item))
                .fold(String::new(), |acc, item| acc + "\n" + &item)
                .trim()
                .to_string();
            let default_target = if targets.len() == 1 {
                format!(r#"CMD ["/app/{}"]"#, targets[0])
            } else {
                "# Default CMD omitted due to multiple targets specified".to_string()
            };

            Ok(format!(
                r#"
# Dockerfile generated by NAIS build (version) at (timestamp)

#
# Builder image
#
FROM {builder_image} AS builder
ENV GOOS=linux
ENV CGO_ENABLED=0
WORKDIR /src

# Copy go.mod and go.sum files into source directory
# so that dependencies can be downloaded before the source code.
# This is a cache optimization step (???)
COPY go.* /src/
RUN go mod download
COPY . /src

# Start hook is run before testing
#RUN ___start_hook

# Test all modules
RUN go test ./...

# Build all binaries found in ./cmd/*
{binary_build_commands}

# End hook is run after build
#RUN ___end_hook

#
# Runtime image
#
FROM {runtime_image}
WORKDIR /app
{binary_copy_commands}
{default_target}
"#,
            ))
        }

        fn filesystem_path(&self) -> String {
            self.0.filesystem_path.clone()
        }

        // fn docker_image_name_tagged(&self) -> String {
        //     self.0.docker_image_name_tagged.clone()
        // }
    }
}
use crate::DetectBuildTargetError::*;
use crate::Error::*;
use clap::{Parser, Subcommand};
use std::io::Write;
use std::process::Stdio;
use thiserror::Error;

#[allow(dead_code)]
mod config;

/// Simple program to greet a person
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
    /// Check if the environment is ready
    Check {
        /// The environment to check
        #[arg(short, long, default_value = "development")]
        environment: String,
    },
    /// Build the Dockerfile
    Dockerfile,
    /// Build builds
    Build,
}

#[derive(Error, Debug)]
enum DetectBuildTargetError {
    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("target name is empty")]
    EmptyFilename,
}

#[derive(Error, Debug)]
enum Error {
    #[error("no compatible SDKs for this source directory")]
    SDKNotDetected,

    #[error("detect build target: {0}")]
    DetectBuildTargetError(#[from] DetectBuildTargetError),

    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("configuration file syntax error: {0}")]
    ConfigDeserialize(#[from] toml::de::Error),
}

/// Read configuration file from disk.
///
/// If a configuration file name is not set explicitly, this function will
/// detect whether a config file with the default file name exists on disk.
/// If it does, it is used implicitly. If not, we ignore any read errors.
fn read_config(args: &Cli) -> Result<config::File, Error> {
    const DEFAULT_CONFIG_FILE: &str = "nb.toml";
    let config_file = match &args.config {
        None => {
            if std::fs::metadata(DEFAULT_CONFIG_FILE)
                .and_then(|metadata| Ok(metadata.is_file()))
                .unwrap_or(false) {
                Some(DEFAULT_CONFIG_FILE.into())
            } else {
                None
            }
        }
        Some(c) => Some(c.clone()),
    };

    Ok(if let Some(config_file) = config_file {
        let config_string = std::fs::read_to_string(&config_file)?;
        toml::from_str::<config::File>(&config_string)?
    } else {
        config::File::default()
    })
}

fn main() -> Result<(), Error> {
    let args = Cli::parse();
    let cfg = read_config(&args)?;

    match args.command {
        Commands::Check { environment } => {
            println!("hello {}", environment);
            Ok(())
        }
        Commands::Dockerfile => {
            let sdk = init_sdk(&args.source_directory, &cfg)?;
            println!("{}", sdk.dockerfile()?);
            Ok(())
        }
        Commands::Build => {
            let sdk = init_sdk(&args.source_directory, &cfg)?;
            let mut file = tempfile::NamedTempFile::new()?;
            file.write_all(sdk.dockerfile()?.as_bytes())?;

            let output = std::process::Command::new("docker")
                .arg("build")
                .arg("--file")
                .arg(file.path())
                .arg(&args.source_directory)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            println!("{:?}", output);
            Ok(())
        }
    }
}

/// SDK is anything that can produce artifacts
trait SDK {
    fn builder_docker_image(&self) -> String;
    fn runtime_docker_image(&self) -> String;
    fn detect_build_targets(&self) -> Result<Vec<String>, DetectBuildTargetError>;
    fn dockerfile(&self) -> Result<String,Error>;
}

fn init_sdk(filesystem_path: &str, cfg: &config::File) -> Result<Box<dyn SDK>, Error> {
    match Golang::new(filesystem_path, cfg) {
        Ok(Some(sdk)) => { return Ok(Box::new(sdk)) }
        Ok(None) => {}
        Err(err) => { return Err(err) }
    }
    Err(SDKNotDetected)
}

struct Golang {
    filesystem_path: String,
    docker_builder_image: String,
    docker_runtime_image: String,
    #[allow(dead_code)]
    start_hook: Option<String>,
    #[allow(dead_code)]
    end_hook: Option<String>,
}

impl Golang {
    fn new(filesystem_path: &str, cfg: &config::File) -> Result<Option<Self>, Error> {
        let Ok(file_stat) = std::fs::metadata(filesystem_path.to_owned() + "/go.mod") else {
            return Ok(None);
        };
        if !file_stat.is_file() {
            return Ok(None);
        }
        Ok(Some(Self {
            filesystem_path: filesystem_path.into(),
            docker_builder_image: cfg.sdk.go.build_docker_image.clone(),
            docker_runtime_image: cfg.sdk.go.runtime_docker_image.clone(),
            start_hook: None,
            end_hook: None,
        }))
    }
}

impl SDK for Golang {
    fn builder_docker_image(&self) -> String {
        self.docker_builder_image.clone()
    }

    fn runtime_docker_image(&self) -> String {
        self.docker_runtime_image.clone()
    }

    /// Return a list of binaries that can be built.
    fn detect_build_targets( &self, ) -> Result<Vec<String>, DetectBuildTargetError> {
        std::fs::read_dir(self.filesystem_path.to_owned() + "/cmd")?
            .map(|dir_entry| {
                Ok(dir_entry?
                    .file_name()
                    .to_str()
                    .ok_or(EmptyFilename)?
                    .to_string())
            })
            .collect()
    }

    fn dockerfile(&self) -> Result<String,Error> {
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
}

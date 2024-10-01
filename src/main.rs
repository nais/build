use crate::DetectBuildTargetError::*;
use crate::Error::*;
use crate::Language::*;
use clap::{Parser, Subcommand};
use std::io::Write;
use std::process::Stdio;
use thiserror::Error;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = ".")]
    source_directory: String,

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

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    match args.command {
        Commands::Check { environment } => {
            println!("hello {}", environment);
            Ok(())
        }
        Commands::Dockerfile => {
            let sdk = detect_sdk(&args.source_directory)?;
            let dockerfile = GolangDockerBuilder::new(sdk, &args.source_directory)?;
            println!("{}", dockerfile.dockerfile());
            Ok(())
        }
        Commands::Build => {
            let sdk = detect_sdk(&args.source_directory)?;
            let dockerfile = GolangDockerBuilder::new(sdk, &args.source_directory)?;
            let mut file = tempfile::NamedTempFile::new()?;
            file.write_all(dockerfile.dockerfile().as_bytes())?;

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

enum Language {
    Go,
}

#[allow(dead_code)]
/// SDK is Language/Framework + Version
struct SDK {
    language: Language,
    version: String,
    //build_image: String,
    //runtime_image: String,
}

impl SDK {
    fn builder_docker_image(&self) -> String {
        match self.language {
            Go => "golang:1-alpine".into(),
        }
    }

    fn runtime_docker_image(&self) -> String {
        match self.language {
            Go => "gcr.io/distroless/static-debian12:nonroot".into(),
            // or gcr.io/distroless/base
        }
    }

    /// Return a list of binaries that can be built.
    fn detect_build_targets(
        &self,
        filesystem_path: &str,
    ) -> Result<Vec<String>, DetectBuildTargetError> {
        std::fs::read_dir(filesystem_path.to_owned() + "/cmd")?
            .map(|dir_entry| {
                Ok(dir_entry?
                    .file_name()
                    .to_str()
                    .ok_or(EmptyFilename)?
                    .to_string())
            })
            .collect()
    }
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
}

fn detect_go(filesystem_path: &str) -> Option<SDK> {
    let file_stat = std::fs::metadata(filesystem_path.to_owned() + "/go.mod").ok()?;
    if !file_stat.is_file() {
        return None;
    }
    Some(SDK {
        language: Go,
        version: "1".into(),
    })
}

fn detect_sdk(filesystem_path: &str) -> Result<SDK, Error> {
    if let Some(sdk) = detect_go(filesystem_path) {
        return Ok(sdk);
    }
    Err(SDKNotDetected)
}

struct GolangDockerBuilder {
    builder_image: String,
    runtime_image: String,
    #[allow(dead_code)]
    start_hook: Option<String>,
    #[allow(dead_code)]
    end_hook: Option<String>,
    targets: Vec<String>,
}

impl GolangDockerBuilder {
    fn new(sdk: SDK, filesystem_path: &str) -> Result<Self, Error> {
        let targets = sdk.detect_build_targets(filesystem_path)?;

        Ok(Self {
            targets,
            builder_image: sdk.builder_docker_image(),
            runtime_image: sdk.runtime_docker_image(),
            start_hook: None,
            end_hook: None,
        })
    }

    fn dockerfile(&self) -> String {
        let builder_image = &self.builder_image;
        let runtime_image = &self.runtime_image;
        let binary_build_commands: String = self
            .targets
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
        let binary_copy_commands: String = self
            .targets
            .iter()
            .map(|item| format!("COPY --from=builder /build/{} /app/{}", item, item))
            .fold(String::new(), |acc, item| acc + "\n" + &item)
            .trim()
            .to_string();
        let default_target = if self.targets.len() == 1 {
            format!(r#"CMD ["/app/{}"]"#, self.targets[0])
        } else {
            "# Default CMD omitted due to multiple targets specified".to_string()
        };

        format!(
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
        )
    }
}

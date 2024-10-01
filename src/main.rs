use clap::{Parser, Subcommand};
use thiserror::Error;
use crate::DetectBuildTargetError::*;
use crate::Error::*;
use crate::Language::*;

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
            let dockerfile = DockerBuildParams::new(sdk, &args.source_directory)?;
            println!("{}", dockerfile.dockerfile());
            Ok(())
        }
        Commands::Build => Ok(()),
    }
}


enum Language {
    Go
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
            Go => "golang:1".into(),
        }
    }

    fn runtime_docker_image(&self) -> String {
        match self.language {
            Go => "golang:1".into(),
        }
    }

    /// Return a list of binaries that can be built.
    fn detect_build_targets(&self, filesystem_path: &str) -> Result<Vec<String>, DetectBuildTargetError> {
        std::fs::read_dir(filesystem_path.to_owned() + "/cmd")?
            .map(|dir_entry|
                Ok(dir_entry?
                    .file_name().to_str().ok_or(EmptyFilename)?
                    .to_string()))
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
    DetectBuildTargetError(#[from] DetectBuildTargetError)
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

struct DockerBuildParams {
    builder_image: String,
    runtime_image: String,
    #[allow(dead_code)]
    start_hook: Option<String>,
    #[allow(dead_code)]
    end_hook: Option<String>,
    binaries: Vec<String>,
}

impl DockerBuildParams {
    fn new(sdk: SDK, filesystem_path: &str) -> Result<Self, Error> {
        let binaries = sdk.detect_build_targets(filesystem_path)?;

        Ok(Self {
            binaries,
            builder_image: sdk.builder_docker_image(),
            runtime_image: sdk.runtime_docker_image(),
            start_hook: None,
            end_hook: None,
        })
    }
    fn dockerfile(&self) -> String {
        let builder_image = &self.builder_image;
        let runtime_image = &self.runtime_image;
        let binary_commands: String = self
            .binaries
            .iter()
            .map(|item| {
                format!(
                    "RUN go build -a -installsuffix cgo -o /build/{} cmd/{}",
                    item, item
                )
            })
            .fold(String::new(), |acc, item| acc + "\n" + &item);
        let binary_copy_commands: String = self
            .binaries
            .iter()
            .map(|item| format!("COPY --from=builder /build/{} /app/{}", item, item))
            .fold(String::new(), |acc, item| acc + "\n" + &item);

        format!(
            r#"
FROM {builder_image} AS builder
#RUN ___start_hook
ENV GOOS=linux
ENV CGO_ENABLED=0
WORKDIR /src
COPY go.* /src/
RUN go mod download
COPY . /src
RUN go test ./...
{binary_commands}
#RUN ___end_hook

FROM {runtime_image}
WORKDIR /app
{binary_copy_commands}
CMD ["/app/naiserator"]
"#,
        )
    }
}

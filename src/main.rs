/// NAIS Build
use crate::Error::*;
use clap::{Parser, Subcommand};
use std::io::Write;
use std::process::{ExitStatus, Stdio};
use thiserror::Error;
use log::{error, info};
use sdk::DockerFileBuilder;
use crate::config::file::{ReleaseParams, ReleaseType};
use crate::nais_yaml::NaisYaml;

#[allow(dead_code)]
mod config;
mod nais_yaml;
mod sdk;

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
pub enum Error {
    #[error("no compatible SDKs for this source directory")]
    SDKNotDetected,

    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("configuration file: {0}")]
    ConfigParse(#[from] config::file::Error),

    #[error("configuration: {0}")]
    Config(#[from] REALError),

    #[error("docker tag could not be generated: {0}")]
    DockerTag(#[from] config::docker::tag::Error),

    #[error("docker build failed with exit code {0}")]
    DockerBuild(ExitStatus),

    #[error("detect nais.yaml: {0}")]
    DetectNaisYaml(#[from] nais_yaml::Error),

    #[error("build error: {0}")]
    SDKError(#[from] sdk::Error),
}

/// Read configuration file from disk and merge it with the
/// `default.toml` [built-in config](../default.toml).
///
/// If a configuration file name is not set explicitly, this function will
/// detect whether a config file with the default file name exists on disk.
/// If it does, it is used implicitly. If not, we ignore any read errors.
fn read_config(args: &Cli) -> Result<config::file::File, Error> {
    const DEFAULT_CONFIG_FILE: &str = "nb.toml";

    // Typically found in project root, e.g. ./nb.toml
    let config_path = format!("{}/{}", args.source_directory, DEFAULT_CONFIG_FILE);

    let config_file = match &args.config {
        None => {
            if std::fs::metadata(&config_path)
                .and_then(|metadata| Ok(metadata.is_file()))
                .unwrap_or(false)
            {
                Some(config_path.into())
            } else {
                None
            }
        }
        Some(c) => Some(c.clone()),
    };

    Ok(if let Some(config_file) = config_file {
        config::file::File::default_with_user_config_file(&config_file)?
    } else {
        config::file::File::default()
    })
}

fn main() {
    match run() {
        Ok(_) => std::process::exit(0),
        Err(err) => {
            error!("fatal: {}", err.to_string());
            std::process::exit(1)
        }
    }
}

struct Release {
    pub typ: ReleaseType,
    pub params: ReleaseParams,
}

impl Release {
    pub fn docker_name_builder(&self, config: config::docker::name::Config) -> Box<dyn ToString> {
        match self.typ {
            ReleaseType::GAR => Box::new(config::docker::name::GoogleArtifactRegistry(config)),
            ReleaseType::GHCR => Box::new(config::docker::name::GitHubContainerRegistry(config)),
        }
    }
}

struct REALConfig {
    app: String,
    team: String,
    release: Release,
}

#[derive(Debug, Clone, Error)]
pub enum REALError {
    #[error("missing configuration")]
    MissingConfig,
}

impl REALConfig {
    fn new(cfg: &config::file::File, nais_yaml: NaisYaml) -> Result<REALConfig, REALError> {
        let release = cfg.release.clone().ok_or(REALError::MissingConfig)?;
        let release_params = release.value();
        Ok(REALConfig {
            app: nais_yaml.app,
            team: cfg.team.clone().unwrap_or(nais_yaml.team),
            release: Release {
                typ: release.typ,
                params: release_params,
            },
        })
    }
}

fn run() -> Result<(), Error> {
    env_logger::init();

    let args = Cli::parse();
    let cfg_file = read_config(&args)?;

    info!("NAIS build 1.0.0");

    let nais_yaml_path = nais_yaml::detect_nais_yaml(&args.source_directory)?;
    info!("nais.yaml detected at {nais_yaml_path}");

    let nais_yaml_data = NaisYaml::parse_file(&nais_yaml_path)?;

    let cfg = REALConfig::new(&cfg_file, nais_yaml_data).map_err(Config)?;

    info!("Application name detected: {}", &cfg.app);
    info!("Team detected: {}", &cfg.team);

    let docker_image_name = cfg
        .release
        .docker_name_builder(config::docker::name::Config {
            registry: cfg.release.params.registry.clone(),
            tag: config::docker::tag::generate(&args.source_directory)?,
            team: cfg.team,
            app: cfg.app,
        })
        .to_string();

    match args.command {
        Commands::Dockerfile => {
            let sdk = init_sdk(&args.source_directory, &cfg_file)?;
            println!("{}\n", sdk.dockerfile()?);
            info!("Docker image tag: {}", docker_image_name);
            Ok(())
        }
        Commands::Build => {
            let sdk = init_sdk(&args.source_directory, &cfg_file)?;
            build(sdk, &docker_image_name)?;
            Ok(())
        }
    }
}

fn build(docker_file_builder: Box<dyn DockerFileBuilder>, tag: &str) -> Result<(), Error> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(docker_file_builder.dockerfile()?.as_bytes())?;

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
                Err(DockerBuild(exit_status))
            }
        })?
}

fn init_sdk(
    filesystem_path: &str,
    cfg: &config::file::File,
) -> Result<Box<dyn DockerFileBuilder>, Error> {
    let sdk = cfg.sdk.clone().unwrap();

    match sdk::golang::new(sdk::golang::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: sdk.go.build_docker_image.clone(),
        docker_runtime_image: sdk.go.runtime_docker_image.clone(),
        start_hook: None,
        end_hook: None,
    }) {
        Ok(Some(sdk)) => {
            return Ok(Box::new(sdk));
        }
        Ok(None) => {}
        Err(err) => return Err(Error::from(err)),
    }

    match sdk::gradle::new(sdk::gradle::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: sdk.gradle.build_docker_image.clone(),
        docker_runtime_image: sdk.gradle.runtime_docker_image.clone(),
        settings_file: sdk.gradle.settings_file.clone(),
        start_hook: None,
        end_hook: None,
    }) {
        Ok(Some(sdk)) => {
            return Ok(Box::new(sdk));
        }
        Ok(None) => {}
        Err(err) => return Err(Error::from(err)),
    }

    match sdk::maven::new(sdk::maven::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: sdk.maven.build_docker_image.clone(),
        docker_runtime_image: sdk.maven.runtime_docker_image.clone(),
        start_hook: None,
        end_hook: None,
    }) {
        Ok(Some(sdk)) => {
            return Ok(Box::new(sdk));
        }
        Ok(None) => {}
        Err(err) => return Err(Error::from(err)),
    }
    // try next sdk

    Err(SDKNotDetected)
}

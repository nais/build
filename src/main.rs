/// NAIS Build
use crate::Error::*;
use clap::{Parser, Subcommand};
use std::io::Write;
use std::process::{ExitStatus, Stdio};
use thiserror::Error;
use sdk::DockerFileBuilder;

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

    #[error("configuration file syntax error: {0}")]
    ConfigDeserialize(#[from] toml::de::Error),

    #[error("docker tag could not be generated: {0}")]
    DockerTag(#[from] config::docker::tag::Error),

    #[error("docker build failed with exit code {0}")]
    DockerBuild(ExitStatus),

    #[error("detect nais.yaml: {0}")]
    DetectNaisYaml(#[from] nais_yaml::Error),

    #[error("build error: {0}")]
    SDKError(#[from] sdk::Error),
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

    let nais_yaml_path = match nais_yaml::detect_nais_yaml(&args.source_directory) {
        Ok(value) => Some(value),
        Err(nais_yaml::Error::NaisYamlNotFound) => None,
        Err(e) => return Err(DetectNaisYaml(e)),
    };

   let nais_yaml_data = if let Some(nais_yaml_path) = nais_yaml_path {
        std::fs::read_to_string(&nais_yaml_path)
        .map(|yaml_string| Ok(nais_yaml::NaisYaml::parse(&yaml_string)?))
        .map(|e: Result<nais_yaml::NaisYaml, Error>| e.unwrap())
        .expect("FIXME: team and app needs to be inferred from nais.yaml")
    };

    let docker_image_name = cfg
        .release
        .docker_name_builder(config::docker::name::Config {
            registry: cfg.release.value().registry,
            tag: config::docker::tag::generate(&args.source_directory)?,
            team: nais_yaml_data.team,
            app: nais_yaml_data.app,
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
    match sdk::golang::new(sdk::golang::Config {
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
        Err(err) => return Err(Error::from(err)),
    }
    match sdk::kotlin::new(sdk::kotlin::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: cfg.sdk.kotlin.build_docker_image.clone(),
        docker_runtime_image: cfg.sdk.kotlin.runtime_docker_image.clone(),
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

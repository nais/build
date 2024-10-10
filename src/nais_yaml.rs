use std::fs::DirEntry;
use log::{debug};
use thiserror::Error;
use Error::*;

#[derive(Error, Debug)]
pub enum Error {
    #[error("no suitable file found")]
    NaisYamlNotFound,

    #[error("scan file system: {0}")]
    FileSystem(#[from] std::io::Error),

    #[error("read {path}: {err}")]
    ReadFile {
        err: std::io::Error,
        path: String,
    },

    #[error("deserialize: {0}")]
    Deserialize(#[from] serde_yaml::Error),
}

fn walk_dir(filesystem_path: &str) -> Result<Vec<DirEntry>, std::io::Error> {
    Ok(std::fs::read_dir(filesystem_path)?
        .filter(|e| e.is_ok())
        .map(|e| e.unwrap())
        .collect())
}

// regex
// \.?nais(erator)\.ya?ml
// ^\.nais/.+\.ya?ml
// ^\.nais/(dev|prod)(-(fss|gcp))?\.ya?ml
/// Returns the path of the first and best detected nais.yaml
pub fn detect_nais_yaml(filesystem_path: &str) -> Result<String, Error> {
    let root_dir_files = walk_dir(filesystem_path)?;
    let nais_files = walk_dir(&format!("{}/.nais", filesystem_path)).unwrap_or_default();
    let candidates = vec![
        ".nais.yaml",
        ".nais.yml",
        ".naiserator.yaml",
        ".naiserator.yml",
        "dev-fss.yaml",
        "dev-fss.yml",
        "dev-gcp.yaml",
        "dev-gcp.yml",
        "dev.yml",
        "nais.yaml",
        "nais.yml",
        "naiserator.yaml",
        "naiserator.yml",
        "prod-fss.yaml",
        "prod-fss.yml",
        "prod-gcp.yaml",
        "prod-gcp.yml",
        "prod.yml",
    ];

    debug!("{} files found in project root", root_dir_files.len());
    debug!("{} files found in .nais directory", nais_files.len());

    [root_dir_files, nais_files]
        .iter()
        .flatten()
        .filter(|e| candidates.contains(&e.file_name().to_str().unwrap()))
        .inspect(|nais_yaml_path| {
            let path = nais_yaml_path.path().to_str().unwrap_or_default().to_string();
            debug!("Possible nais.yaml candidate: {path}")
        })
        .collect::<Vec<_>>()
        .iter().next() // Needed in order for inspect() to gather all values
        .ok_or(NaisYamlNotFound)
        .map(|e| e.path().to_str().unwrap().to_string())
}

pub struct NaisYaml {
    pub team: String,
    pub app: String,
}

impl NaisYaml {
    pub fn parse(yaml_string: &str) -> Result<Self, Error> {
        let parsed = serde_yaml::from_str::<yaml::KubernetesResource>(yaml_string)?;
        Ok(Self {
            team: parsed.metadata.namespace,
            app: parsed.metadata.name,
        })
    }

    pub fn parse_file(path: &str) -> Result<Self, Error> {
        Self::parse(
            &std::fs::read_to_string(path).map_err(|err| ReadFile {
                err,
                path: path.to_string(),
            })?
        )
    }
}

mod yaml {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct Metadata {
        pub name: String,
        pub namespace: String,
    }

    #[derive(Deserialize)]
    pub struct KubernetesResource {
        pub metadata: Metadata,
    }
}

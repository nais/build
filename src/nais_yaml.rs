use std::fs::DirEntry;
use thiserror::Error;
use Error::*;

#[derive(Error, Debug)]
pub enum Error {
    #[error("no suitable file found")]
    NaisYamlNotFound,

    #[error("scan file system: {0}")]
    FileSystem(#[from] std::io::Error),

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
// ^\.nais/.+\.yaml
// ^\.nais/(dev|prod)-(fss|gcp)+\.yaml
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
        "nais.yaml",
        "nais.yml",
        "naiserator.yaml",
        "naiserator.yml",
        "prod-fss.yaml",
        "prod-fss.yml",
        "prod-gcp.yaml",
        "prod-gcp.yml",
    ];

    [root_dir_files, nais_files]
        .iter()
        .flatten()
        .filter(|&e| candidates.contains(&e.file_name().to_str().unwrap()))
        .next()
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

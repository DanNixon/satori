use serde::Deserialize;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ConfigFileError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Format error: {0}")]
    Format(#[from] toml::de::Error),
}

pub fn load_config_file<T: for<'de> Deserialize<'de>>(file: &Path) -> Result<T, ConfigFileError> {
    Ok(toml::from_str(&std::fs::read_to_string(file)?)?)
}

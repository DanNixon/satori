use miette::{Context, IntoDiagnostic};
use serde::Deserialize;
use std::path::Path;

pub fn load_config_file<T: for<'de> Deserialize<'de>>(file: &Path) -> miette::Result<T> {
    toml::from_str(
        &std::fs::read_to_string(file)
            .into_diagnostic()
            .wrap_err("Failed to read config file")?,
    )
    .into_diagnostic()
    .wrap_err("Failed to parse config file")
}

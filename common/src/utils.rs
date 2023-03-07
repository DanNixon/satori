use serde::Deserialize;
use std::path::Path;

pub fn load_config_file<T: for<'de> Deserialize<'de>>(file: &Path) -> T {
    toml::from_str(&std::fs::read_to_string(file).expect("config file should be readable"))
        .expect("config file should be valid")
}

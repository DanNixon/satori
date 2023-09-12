#[macro_export]
macro_rules! version {
    () => {
        format!(
            "v{} git={}",
            clap::crate_version!(),
            std::option_env!("GIT_REVISION").unwrap_or("<unknown>")
        )
    };
}

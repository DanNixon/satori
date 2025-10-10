#[macro_export]
macro_rules! version {
    () => {
        format!(
            "v{} git={}",
            clap::crate_version!(),
            git_version::git_version!(),
        )
    };
}

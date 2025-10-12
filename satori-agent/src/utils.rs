use m3u8_rs::MediaPlaylist;
use miette::{Context, IntoDiagnostic};
use std::{fs, path::Path};

pub(super) async fn load_playlist(playlist_filename: &Path) -> miette::Result<MediaPlaylist> {
    let content = tokio::fs::read(playlist_filename)
        .await
        .into_diagnostic()
        .wrap_err("Failed to read HLS playlist file")?;

    satori_common::parse_m3u8_media_playlist(&content)
}

pub(crate) fn get_size<P>(path: P) -> std::io::Result<u64>
where
    P: AsRef<Path>,
{
    let mut result: u64 = 0;

    for entry in fs::read_dir(&path)? {
        let path = entry?.path();

        if path.is_file() {
            result += path.metadata()?.len();
        } else {
            result += get_size(path)?;
        }
    }

    Ok(result)
}

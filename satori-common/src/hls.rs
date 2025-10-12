use m3u8_rs::MediaPlaylist;
use miette::Context;
use tracing::trace;

pub const SEGMENT_FILENAME_FORMAT: &str = "%Y-%m-%dT%H_%M_%S%z.ts";

pub fn parse_m3u8_media_playlist(data: &[u8]) -> miette::Result<MediaPlaylist> {
    let playlist = m3u8_rs::parse_playlist_res(data)
        .map_err(|e| miette::miette!("{e}"))
        .wrap_err("Failed to parse m3u8 playlist")?;

    let playlist = if let m3u8_rs::Playlist::MediaPlaylist(pl) = playlist {
        Ok(pl)
    } else {
        Err(miette::miette!("Incorrect playlist type"))
    }?;

    trace!(
        "Parsed playlist with {} media items",
        playlist.segments.len()
    );

    Ok(playlist)
}

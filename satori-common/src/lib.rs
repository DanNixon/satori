pub mod camera_config;
mod event;
mod message_schema;
pub mod mqtt;
mod trigger;
mod utils;
mod version;

pub use self::{
    event::{CameraSegments, Event, EventMetadata, EventReason},
    message_schema::{ArchiveCommand, ArchiveSegmentsCommand, Message, TriggerCommand},
    trigger::{Trigger, TriggerTemplate},
    utils::{ThrottledErrorLogger, load_config_file},
};

pub const SEGMENT_FILENAME_FORMAT: &str = "%Y-%m-%dT%H_%M_%S%z.ts";

use m3u8_rs::MediaPlaylist;
use miette::Context;
use tracing::trace;

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

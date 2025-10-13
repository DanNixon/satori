use chrono::{DateTime, FixedOffset};
use m3u8_rs::MediaPlaylist;
use miette::Context;
use std::time::Duration;
use tracing::{trace, warn};

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

/// Filter a MediaPlaylist to contain only segments within the specified time range.
///
/// # Arguments
/// * `playlist` - The playlist to filter
/// * `start` - Optional start timestamp (inclusive)
/// * `end` - Optional end timestamp (inclusive)
///
/// If both start and end are None, the playlist is returned unchanged.
pub fn filter_playlist_by_time(
    mut playlist: MediaPlaylist,
    start: Option<DateTime<FixedOffset>>,
    end: Option<DateTime<FixedOffset>>,
) -> miette::Result<MediaPlaylist> {
    // If no filters are provided, return the playlist as-is
    if start.is_none() && end.is_none() {
        return Ok(playlist);
    }

    playlist.segments.retain(|segment| {
        // Parse the segment timestamp from the filename
        let segment_start =
            match DateTime::<FixedOffset>::parse_from_str(&segment.uri, SEGMENT_FILENAME_FORMAT) {
                Ok(dt) => dt,
                Err(e) => {
                    warn!(
                        "Failed to parse datetime from segment filename: {0} ({e})",
                        segment.uri
                    );
                    return false; // Skip segments we can't parse
                }
            };

        let segment_duration = Duration::from_secs_f32(segment.duration);
        let segment_end =
            segment_start + chrono::Duration::from_std(segment_duration).unwrap_or_default();

        // Check if segment overlaps with the requested time range
        let after_start = start.is_none_or(|s| segment_end >= s);
        let before_end = end.is_none_or(|e| segment_start <= e);

        after_start && before_end
    });

    trace!("Filtered playlist to {} segments", playlist.segments.len());

    Ok(playlist)
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::NaiveDate;
    use m3u8_rs::MediaSegment;

    fn create_test_segment(timestamp: &str, duration: f32) -> MediaSegment {
        MediaSegment {
            uri: timestamp.to_string(),
            duration,
            ..Default::default()
        }
    }

    fn create_test_playlist() -> MediaPlaylist {
        MediaPlaylist {
            segments: vec![
                create_test_segment("2022-12-30T18_10_00+0000.ts", 6.0),
                create_test_segment("2022-12-30T18_10_06+0000.ts", 6.0),
                create_test_segment("2022-12-30T18_10_12+0000.ts", 6.0),
                create_test_segment("2022-12-30T18_10_18+0000.ts", 6.0),
                create_test_segment("2022-12-30T18_10_24+0000.ts", 6.0),
            ],
            ..Default::default()
        }
    }

    fn make_datetime(hour: u32, min: u32, sec: u32) -> DateTime<FixedOffset> {
        NaiveDate::from_ymd_opt(2022, 12, 30)
            .unwrap()
            .and_hms_opt(hour, min, sec)
            .unwrap()
            .and_local_timezone(FixedOffset::east_opt(0).unwrap())
            .unwrap()
    }

    #[test]
    fn test_filter_no_filters() {
        let playlist = create_test_playlist();
        let original_count = playlist.segments.len();
        let filtered = filter_playlist_by_time(playlist, None, None).unwrap();
        assert_eq!(filtered.segments.len(), original_count);
    }

    #[test]
    fn test_filter_only_start() {
        let playlist = create_test_playlist();
        let start = make_datetime(18, 10, 10);
        let filtered = filter_playlist_by_time(playlist, Some(start), None).unwrap();
        // Should include segments that end after 18:10:10
        // Segment 1: 18:10:06 - 18:10:12 (overlaps)
        // Segment 2: 18:10:12 - 18:10:18 (included)
        // Segment 3: 18:10:18 - 18:10:24 (included)
        // Segment 4: 18:10:24 - 18:10:30 (included)
        assert_eq!(filtered.segments.len(), 4);
    }

    #[test]
    fn test_filter_only_end() {
        let playlist = create_test_playlist();
        let end = make_datetime(18, 10, 10);
        let filtered = filter_playlist_by_time(playlist, None, Some(end)).unwrap();
        // Should include segments that start before or at 18:10:10
        // Segment 0: 18:10:00 - 18:10:06 (included)
        // Segment 1: 18:10:06 - 18:10:12 (starts before, included)
        assert_eq!(filtered.segments.len(), 2);
    }

    #[test]
    fn test_filter_both_start_and_end() {
        let playlist = create_test_playlist();
        let start = make_datetime(18, 10, 8);
        let end = make_datetime(18, 10, 20);
        let filtered = filter_playlist_by_time(playlist, Some(start), Some(end)).unwrap();
        // Should include segments that overlap with 18:10:08 to 18:10:20
        // Segment 1: 18:10:06 - 18:10:12 (overlaps)
        // Segment 2: 18:10:12 - 18:10:18 (included)
        // Segment 3: 18:10:18 - 18:10:24 (overlaps)
        assert_eq!(filtered.segments.len(), 3);
    }

    #[test]
    fn test_filter_no_matches() {
        let playlist = create_test_playlist();
        let start = make_datetime(18, 11, 0);
        let end = make_datetime(18, 12, 0);
        let filtered = filter_playlist_by_time(playlist, Some(start), Some(end)).unwrap();
        assert_eq!(filtered.segments.len(), 0);
    }
}

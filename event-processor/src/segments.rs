use chrono::{DateTime, FixedOffset};
use std::{path::PathBuf, time::Duration};

pub(crate) struct Playlist {
    pub(crate) segments: Vec<SegmentFile>,
}

impl Playlist {
    pub(crate) fn between(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> Vec<&SegmentFile> {
        self.segments
            .iter()
            .filter(|s| s.between(start, end))
            .collect()
    }
}

impl From<m3u8_rs::MediaPlaylist> for Playlist {
    fn from(playlist: m3u8_rs::MediaPlaylist) -> Self {
        Self {
            segments: playlist.segments.into_iter().map(|i| i.into()).collect(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct SegmentFile {
    pub(crate) filename: PathBuf,

    start: DateTime<FixedOffset>,
    end: DateTime<FixedOffset>,
}

impl SegmentFile {
    pub(crate) fn between(&self, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>) -> bool {
        !((start < self.start && end < self.start) || self.end < start && self.end < end)
    }
}

impl From<m3u8_rs::MediaSegment> for SegmentFile {
    fn from(segment: m3u8_rs::MediaSegment) -> Self {
        let start = DateTime::<FixedOffset>::parse_from_str(
            &segment.uri,
            satori_common::SEGMENT_FILENAME_FORMAT,
        )
        .unwrap();

        let end =
            start + chrono::Duration::from_std(Duration::from_secs_f32(segment.duration)).unwrap();

        Self {
            filename: segment.uri.into(),
            start,
            end,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_test_file() -> SegmentFile {
        SegmentFile {
            filename: Default::default(),
            start: chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 10, 0)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
            end: chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 11, 0)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
        }
    }

    #[test]
    fn test_segment_file_between_1() {
        let file = get_test_file();
        assert!(file.between(
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 9, 30)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 10, 30)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
        ));
    }

    #[test]
    fn test_segment_file_between_2() {
        let file = get_test_file();
        assert!(file.between(
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 10, 30)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 11, 30)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
        ));
    }

    #[test]
    fn test_segment_file_between_not_1() {
        let file = get_test_file();
        assert!(!file.between(
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 12, 0)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 13, 0)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
        ));
    }

    #[test]
    fn test_segment_file_between_not_2() {
        let file = get_test_file();
        assert!(!file.between(
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 8, 0)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
            chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                .unwrap()
                .and_hms_opt(18, 9, 0)
                .unwrap()
                .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                .unwrap(),
        ));
    }
}

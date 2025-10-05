pub mod camera_config;

mod event;
pub use self::event::{CameraSegments, Event, EventMetadata, EventReason};

mod message_schema;
pub use self::message_schema::{ArchiveCommand, ArchiveSegmentsCommand, Message, TriggerCommand};

pub mod mqtt;

mod trigger;
pub use self::trigger::{Trigger, TriggerTemplate};

pub const SEGMENT_FILENAME_FORMAT: &str = "%Y-%m-%dT%H_%M_%S%z.ts";

mod version;

mod utils;
pub use self::utils::{ThrottledErrorLogger, load_config_file};

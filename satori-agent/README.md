# Agent

The agent (better name wanted) is the component that bridges a camera (or any Ffmpeg supported video source) to Satori.

In the simplest sense, it is a fancy wrapper around the `ffmpeg` command line which is used to convert whatever the camera provides into a HLS stream.

It has the following responsibilities:

- Recording video from a single camera
- Storing said video for a given retention period
- Providing a HLS endpoint to access the stored video
- Providing an HTTP endpoint for a image/MJPEG stream
- Providing a very basic browser based video player for it's camera's video
- Providing some basic health metrics

## Configuration

There should be one instance of the agent for every camera in the system.

An example config file is shown below:

```toml
# The directory in which recorded video will be saved.
# This can either be persistent storage if you want resilience in the event of
# power failure or want a long history of consistent recording, or
# volatile/in-memory (e.g. tmpfs or ramfs) if you only care about as much video
# as you can fit in memory and can live with the loss of video on power cycle.
video_directory = "/mnt/video/this-camera"

[stream]
# The URL of the video source as passed to `ffmpeg`.
# See `supported-cameras.md` for known working settings for specific cameras.
url = "http://<this-camera>/flv?port=1935&app=bcs&stream=channel0_main.bcs&user=<user>&password=<pass>"

# Extra arguments passed to the input of `ffmpeg`.
# See `supported-cameras.md` for known working settings for specific cameras.
ffmpeg_input_args = [
  "-timeout",
  "5000000",
  "-avoid_negative_ts",
  "make_zero",
  "-fflags",
  "+genpts+discardcorrupt",
  "-flags",
  "low_delay",
  "-strict",
  "experimental",
]

# Duration in seconds of a HLS segment.
# Shorter segments will reduce the latency of the HLS stream.
hls_segment_time = 6

# Number of HLS segments to retain.
# This will determine the duration of video that is retained (i.e. 14400 (hls_retained_segment_count) * 6 (hls_segment_time) = 86400 (1 day)).
hls_retained_segment_count = 14400
```

## HTTP API

The following endpoints are available on the HTTP server address of a running agent:

- `/jpeg`: a single frame in JPEG format, updated every second
- `/mjpeg`: an MJPEG stream, updated every second
- `/hls`: HLS stream for the recorded video (supports time-based filtering, see below)
- `/player`: a basic browser based player for the HLS stream

### HLS Endpoint Query Parameters

The `/hls` endpoint supports optional query parameters to filter the returned playlist by time:

- `since`: Start timestamp in RFC3339 format (e.g., `2022-12-30T18:10:00+00:00`). Only segments that end at or after this time are included.
- `until`: End timestamp in RFC3339 format (e.g., `2022-12-30T18:20:00+00:00`). Only segments that start at or before this time are included.
- `last`: Duration in the past (e.g., `10s`, `5m`, `1h`, `30m`). Only segments from the last N time are included. This parameter cannot be used together with `since` or `until`.

Examples:
- `/hls?since=2022-12-30T18:10:00+00:00` - Get all segments from 18:10:00 onwards
- `/hls?until=2022-12-30T18:20:00+00:00` - Get all segments up to 18:20:00
- `/hls?since=2022-12-30T18:10:00+00:00&until=2022-12-30T18:20:00+00:00` - Get segments between 18:10:00 and 18:20:00
- `/hls?last=5m` - Get segments from the last 5 minutes
- `/hls` - Get all available segments (no filtering)

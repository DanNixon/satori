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
# video_directory = "/mnt/video/this-camera".

[stream]
# The URL of the video source as passed to `ffmpeg`.
# (this example works well for Reolink PoE cameras)
url = "http://<this-camera>/flv?port=1935&app=bcs&stream=channel0_main.bcs&user=<user>&password=<pass>"

# Extra arguments passed to the input of `ffmpeg`.
# (this example works well for Reolink PoE cameras)
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

- `frame.jpg`: a single frame in JPEG format, updated every second
- `/stream.m3u8`: HLS stream for the cache of recorded video
- `player`: a basic browser based player for the HLS stream

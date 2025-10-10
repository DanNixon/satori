# Supported/Tested cameras

This lists some known working configurations for specific cameras.
However, any video feed that is supported by ffmpeg will just work.
Having `ffplay` correctly and reliably display the stream from your camera is a good indication that it will work well with Satori.

## [Reolink RLC-520A](https://reolink.com/product/rlc-520a/)

(has also been used on several other 5MP Reolink cameras)

```toml
[stream]
url = "https://<HOST>/flv?port=1935&app=bcs&stream=channel0_main.bcs&user=<USER>&password=<PASS>"
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
hls_segment_time = 6
hls_retained_segment_count = 14400
```

## [Reolink RLC-820A](https://reolink.com/product/rlc-820a/)

```toml
[stream]
url = "rtsp://<USER>:<PASS>@<HOST>/Preview_01_main"
ffmpeg_input_args = [
  "-fflags",
  "+genpts+discardcorrupt",
  "-flags",
  "low_delay",
  "-strict",
  "experimental",
]
hls_segment_time = 6
hls_retained_segment_count = 14400
```

## [Unifi UVC G3 Flex](https://uk.store.ui.com/uk/en/products/uvc-g3-flex)

```toml
[stream]
url = "rtsp://admin:asdf1234@10.1.11.63/Preview_01_main"
url = "rtsp://<USER>:<PASS>@<HOST>/s0"
ffmpeg_input_args = [
  "-fflags",
  "+genpts+discardcorrupt",
  "-flags",
  "low_delay",
]
hls_segment_time = 6
hls_retained_segment_count = 14400
```

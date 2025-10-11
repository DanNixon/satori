# satori

[![CI](https://github.com/DanNixon/satori/actions/workflows/ci.yml/badge.svg)](https://github.com/DanNixon/satori/actions/workflows/ci.yml)

Satori is an opinionated, component-based Network Video Recorder (NVR) for IP cameras, built with the UNIX philosophy in mind: *Do One Thing and Do It Well*.

## Overview

Satori is designed to continuously record all connected cameras, retaining this footage for a configurable period.
When an external system triggers an "event" (e.g., a motion sensor, on-camera or external ML detector), Satori archives the relevant video from the specified cameras for long-term storage.

The system relies on `ffmpeg` for video processing, allowing it to support a wide range of cameras and video sources.

## Documentation

Full documentation for the project can be found in the [docs](./docs) directory.

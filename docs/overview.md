# Overview

Satori is a Network Video Recorder (NVR) primarily designed for network accessible CCTV cameras.
Much of the video handling uses [`ffmpeg`](https://ffmpeg.org/) behind the scenes, so source device support is largely limited only by what can be made to work with `ffmpeg`.

Each NVR does things slightly differently, some allowing a large degree of freedom as to how recording and archiving work.
Satori is very opinionated, i.e. it only really has one mode of operation.

## Glossary of Terms

- Trigger: an indication from an external system that something interesting has happened in view of one or many (or none) cameras
- Event: a timespan in which something "interesting" has happened for which video should be retained/archived
- Segment: an MPEG-TS segment (ideally this would be more of an implementation detail than it actually is, but this is user visible in several areas)

## Mode of operation

Satori operates under the following principles:

- Record all cameras, all the time, retaining this "raw" recording for some period of time
- When an event occurs, archive the relevant time window of video from the relevant cameras somewhere

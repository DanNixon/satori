# Architecture

Satori is designed with the [UNIX philosophy](https://en.wikipedia.org/wiki/Unix_philosophy#Do_One_Thing_and_Do_It_Well) in mind.
As such a complete system has several components, each with their own specific responsibility.

Each component is fully described below:

- [Agent](../satori-agent/)
- [Event Processor](../satori-event-processor/)
- [Archiver](../satori-archiver/)

Communication between components is performed via either HTTP or [MQTT](https://wikipedia.org/wiki/MQTT) for signalling and [HTTP Live Streaming](https://wikipedia.org/wiki/HTTP_Live_Streaming) for media transport.

## Diagram

```mermaid
graph LR
    CCTV1[Camera 1]
    CCTV2[Camera 2]
    CCTVN[Camera N]

    User[User/System]

    subgraph "Recording"
        direction TB
        Agent1[satori-agent 1]
        Agent2[satori-agent 2]
        AgentN[satori-agent N]
        Agent1 --- Agent1_Disk[Storage]
        Agent2 --- Agent2_Disk[Storage]
        AgentN --- AgentN_Disk[Storage]
    end

    MQTT[MQTT Broker]

    subgraph "Event Processing"
        direction TB
        EventProcessor[satori-event-processor]
        EventProcessor --- EP_Disk[Storage]
    end

    subgraph "Event Archiving"
        direction TB
        Archiver1[satori-archiver 1]
        Archiver2[satori-archiver 2]
        LocalDisk[Local Disk]
    end

    S3[S3 Bucket]

    CCTV1 -- e.g. RTSP/RTMP --> Agent1
    CCTV2 -- e.g. RTSP/RTMP --> Agent2
    CCTVN -- e.g. RTSP/RTMP --> AgentN

    User -- HTTP Event Trigger --> EventProcessor
    EventProcessor -- Archive Command --> MQTT

    MQTT -- Archive Command --> Archiver1
    MQTT -- Archive Command --> Archiver2

    Agent1 -- HLS --> Archiver1
    Agent2 -- HLS --> Archiver2
    AgentN -- HLS --> Archiver2

    Archiver1 --> LocalDisk
    Archiver2 --> S3
```

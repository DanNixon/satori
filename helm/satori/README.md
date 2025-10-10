# Satori Helm Chart

This Helm chart deploys the Satori NVR system to Kubernetes.

## Overview

Satori is a Network Video Recorder (NVR) designed for IP cameras with the following components:

- **satori-agent**: Handles video streaming from cameras (one instance per camera)
- **satori-event-processor**: Processes event triggers and coordinates recording
- **satori-archiver**: Archives video segments to storage (one instance per storage target)

## Prerequisites

- Kubernetes 1.19+
- Helm 3.0+
- PV provisioner support in the underlying infrastructure (for persistent storage)
- An MQTT broker (e.g., Mosquitto)

## Installing the Chart

To install the chart with the release name `my-satori`:

```bash
helm install my-satori ./helm/satori
```

Or with custom values:

```bash
helm install my-satori ./helm/satori -f my-values.yaml
```

## Uninstalling the Chart

To uninstall/delete the `my-satori` deployment:

```bash
helm uninstall my-satori
```

## Configuration

### Global Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `image.registry` | Container image registry | `ghcr.io/dannixon` |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |
| `image.tag` | Image tag | `main` |

### MQTT Configuration

All components share the same MQTT configuration:

| Parameter | Description | Default |
|-----------|-------------|---------|
| `mqtt.broker` | MQTT broker hostname | `mosquitto` |
| `mqtt.port` | MQTT broker port | `1883` |
| `mqtt.username` | MQTT username | `satori` |
| `mqtt.password` | MQTT password | `""` |
| `mqtt.topic` | MQTT topic | `satori` |

### Camera Configuration

Cameras are defined as a list under `cameras`. Each camera creates a separate `satori-agent` instance:

```yaml
cameras:
  - name: front-door
    url: "rtsp://camera.local/stream"
    videoDirectory: /data/video
    ffmpegInputArgs:
      - "-rtsp_transport"
      - "tcp"
    hlsSegmentTime: 6
    hlsRetainedSegmentCount: 600
    ffmpegRestartDelay: 5
    storage:
      storageClassName: ""
      size: 100Gi
      accessMode: ReadWriteOnce
    service:
      type: ClusterIP
      httpPort: 8000
      metricsPort: 9090
    resources: {}
```

### Event Processor Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `eventProcessor.enabled` | Enable event processor | `true` |
| `eventProcessor.eventFile` | Path to event state file | `/data/events.json` |
| `eventProcessor.interval` | Processing interval (seconds) | `10` |
| `eventProcessor.eventTtl` | Event TTL (seconds) | `300` |
| `eventProcessor.triggers.fallback` | Default trigger template | See values.yaml |
| `eventProcessor.triggers.templates` | Named trigger templates | `{}` |

#### Trigger Templates

Trigger templates define how events are handled:

```yaml
eventProcessor:
  triggers:
    fallback:
      cameras:
        - camera-1
      reason: "Unknown event"
      pre: 60    # seconds before event
      post: 120  # seconds after event
    templates:
      motion-detection:
        cameras:
          - front-door
        reason: "Motion detected"
        pre: 30
        post: 60
```

### Archiver Configuration

Archivers are defined as a list under `archivers`. Each archiver creates a separate `satori-archiver` instance:

```yaml
archivers:
  - name: s3-primary
    enabled: true
    queueFile: /data/queue.json
    interval: 100  # milliseconds
    storage:
      kind: s3
      bucket: "satori"
      region: "us-east-1"
      endpoint: ""
    env:
      - name: AWS_ACCESS_KEY_ID
        valueFrom:
          secretKeyRef:
            name: s3-credentials
            key: access-key-id
      - name: AWS_SECRET_ACCESS_KEY
        valueFrom:
          secretKeyRef:
            name: s3-credentials
            key: secret-access-key
```

Supported storage kinds:
- `s3`: Amazon S3 or compatible (MinIO, etc.)
- `local`: Local filesystem storage

## Examples

### Basic Configuration with Two Cameras

```yaml
mqtt:
  broker: "mosquitto.default.svc.cluster.local"
  port: 1883
  username: "satori"
  password: "secret"

cameras:
  - name: front-door
    url: "rtsp://192.168.1.10/stream"
    storage:
      size: 100Gi
  
  - name: back-yard
    url: "rtsp://192.168.1.11/stream"
    storage:
      size: 100Gi

eventProcessor:
  triggers:
    templates:
      motion-detection:
        cameras:
          - front-door
          - back-yard
        reason: "Motion detected"
        pre: 30
        post: 60

archivers:
  - name: s3-archive
    enabled: true
    storage:
      kind: s3
      bucket: "my-nvr-archive"
      region: "us-west-2"
    env:
      - name: AWS_ACCESS_KEY_ID
        valueFrom:
          secretKeyRef:
            name: aws-credentials
            key: access-key-id
      - name: AWS_SECRET_ACCESS_KEY
        valueFrom:
          secretKeyRef:
            name: aws-credentials
            key: secret-access-key
```

### Multiple Archive Targets

```yaml
archivers:
  - name: s3-primary
    enabled: true
    storage:
      kind: s3
      bucket: "primary-archive"
      region: "us-east-1"
    env:
      - name: AWS_ACCESS_KEY_ID
        valueFrom:
          secretKeyRef:
            name: s3-primary-creds
            key: access-key-id
      - name: AWS_SECRET_ACCESS_KEY
        valueFrom:
          secretKeyRef:
            name: s3-primary-creds
            key: secret-access-key
  
  - name: local-backup
    enabled: true
    storage:
      kind: local
      path: /backup
```

## Accessing Services

### Event Processor Trigger API

The event processor exposes a trigger API at `/trigger`:

```bash
# Port forward to local machine
kubectl port-forward svc/my-satori-event-processor 8000:8000

# Send a trigger
curl -X POST http://localhost:8000/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "id": "motion-detection",
    "reason": "Motion detected at front door"
  }'
```

### Camera Agent Web UI

Each camera agent exposes a web UI for viewing the live stream:

```bash
# Port forward to local machine
kubectl port-forward svc/my-satori-agent-front-door 8000:8000

# Access the player at http://localhost:8000/player
```

### Metrics

All components expose Prometheus metrics on port 9090:

```bash
kubectl port-forward svc/my-satori-event-processor 9090:9090
curl http://localhost:9090/metrics
```

## Troubleshooting

### Check Pod Status

```bash
kubectl get pods -l app.kubernetes.io/name=satori
```

### View Logs

```bash
# Event processor logs
kubectl logs -l app.kubernetes.io/component=event-processor

# Agent logs
kubectl logs -l satori.io/camera=front-door

# Archiver logs
kubectl logs -l satori.io/archiver=s3-primary
```

### Check Configuration

```bash
# View agent configuration
kubectl get configmap my-satori-agent-front-door -o yaml

# View event processor configuration
kubectl get configmap my-satori-event-processor -o yaml
```

## License

See the main Satori repository for license information.

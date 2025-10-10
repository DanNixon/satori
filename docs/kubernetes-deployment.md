# Kubernetes Deployment with Helm

This guide explains how to deploy Satori to Kubernetes using the provided Helm chart.

## Overview

The Satori Helm chart provides a declarative way to deploy all components of the Satori NVR system to a Kubernetes cluster. The chart handles:

- **Multiple camera agents**: One StatefulSet per camera with persistent storage for video segments
- **Event processor**: Central coordinator for event triggers and video archiving
- **Multiple archivers**: One Deployment per archive target (S3, local storage, etc.)
- **Shared configuration**: Common MQTT settings across all components
- **Service exposure**: Each agent and the event processor get their own Service

## Prerequisites

- Kubernetes cluster (1.19+)
- Helm 3.0+
- Storage provisioner for PersistentVolumes
- MQTT broker (e.g., Mosquitto)
- Access to container images (ghcr.io/dannixon/satori-*)

## Quick Start

### 1. Deploy an MQTT Broker

If you don't have an MQTT broker, you can deploy Mosquitto:

```bash
# Create namespace
kubectl create namespace satori

# Deploy Mosquitto
kubectl apply -f - <<EOF
apiVersion: v1
kind: ConfigMap
metadata:
  name: mosquitto-config
  namespace: satori
data:
  mosquitto.conf: |
    listener 1883
    allow_anonymous true
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mosquitto
  namespace: satori
spec:
  replicas: 1
  selector:
    matchLabels:
      app: mosquitto
  template:
    metadata:
      labels:
        app: mosquitto
    spec:
      containers:
      - name: mosquitto
        image: eclipse-mosquitto:2
        ports:
        - containerPort: 1883
        volumeMounts:
        - name: config
          mountPath: /mosquitto/config/mosquitto.conf
          subPath: mosquitto.conf
      volumes:
      - name: config
        configMap:
          name: mosquitto-config
---
apiVersion: v1
kind: Service
metadata:
  name: mosquitto
  namespace: satori
spec:
  selector:
    app: mosquitto
  ports:
  - port: 1883
    targetPort: 1883
EOF
```

### 2. Create a Values File

Create a file named `my-values.yaml` with your camera and storage configuration:

```yaml
mqtt:
  broker: "mosquitto.satori.svc.cluster.local"
  port: 1883
  username: "satori"
  password: ""

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

### 3. Create Secrets

If using S3, create a secret for AWS credentials:

```bash
kubectl create secret generic aws-credentials \
  --namespace=satori \
  --from-literal=access-key-id=YOUR_ACCESS_KEY \
  --from-literal=secret-access-key=YOUR_SECRET_KEY
```

### 4. Install the Chart

```bash
helm install satori ./helm/satori \
  --namespace=satori \
  --create-namespace \
  -f my-values.yaml
```

### 5. Verify Deployment

```bash
# Check all pods are running
kubectl get pods -n satori

# Check services
kubectl get svc -n satori
```

## Configuration

### Camera Configuration

Each camera in the `cameras` list creates a separate agent instance:

```yaml
cameras:
  - name: front-door              # Unique camera name
    url: "rtsp://camera/stream"   # RTSP stream URL
    videoDirectory: /data/video    # Directory for video segments
    ffmpegInputArgs:               # FFmpeg input arguments
      - "-rtsp_transport"
      - "tcp"
    hlsSegmentTime: 6              # Segment duration (seconds)
    hlsRetainedSegmentCount: 600   # Number of segments to keep
    ffmpegRestartDelay: 5          # Delay before restarting FFmpeg
    storage:
      storageClassName: ""         # Storage class for PVC
      size: 100Gi                  # PVC size
      accessMode: ReadWriteOnce
    service:
      type: ClusterIP
      httpPort: 8000
      metricsPort: 9090
    resources:
      limits:
        cpu: 2000m
        memory: 2Gi
      requests:
        cpu: 1000m
        memory: 1Gi
```

### Event Processor Configuration

The event processor coordinates event recording:

```yaml
eventProcessor:
  enabled: true
  eventFile: /data/events.json
  interval: 10                    # Processing interval (seconds)
  eventTtl: 300                   # Event TTL (seconds)
  
  triggers:
    fallback:                     # Default trigger
      cameras:
        - front-door
        - back-yard
      reason: "Unknown event"
      pre: 60                     # Seconds before event
      post: 120                   # Seconds after event
    
    templates:                    # Named trigger templates
      motion-detection:
        cameras:
          - front-door
        reason: "Motion detected"
        pre: 30
        post: 60
```

### Archiver Configuration

Multiple archivers can be configured for different storage targets:

```yaml
archivers:
  - name: s3-primary
    enabled: true
    queueFile: /data/queue.json
    interval: 100                 # Processing interval (milliseconds)
    
    storage:
      kind: s3                    # Storage type: s3 or local
      bucket: "satori"
      region: "us-east-1"
      endpoint: ""                # For S3-compatible (MinIO, etc.)
    
    env:                          # Environment variables
      - name: AWS_ACCESS_KEY_ID
        valueFrom:
          secretKeyRef:
            name: s3-credentials
            key: access-key-id
```

### MQTT Configuration

MQTT settings are shared across all components:

```yaml
mqtt:
  broker: "mosquitto.default.svc.cluster.local"
  port: 1883
  username: "satori"
  password: ""
  topic: "satori"
```

## Accessing Services

### Event Processor Trigger API

Forward the service port to trigger events:

```bash
kubectl port-forward -n satori svc/satori-event-processor 8000:8000

# Send a trigger
curl -X POST http://localhost:8000/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "id": "motion-detection",
    "reason": "Motion detected at front door"
  }'
```

### Camera Agent Web UI

Access the live stream player for a camera:

```bash
kubectl port-forward -n satori svc/satori-agent-front-door 8000:8000

# Open http://localhost:8000/player in your browser
```

### Prometheus Metrics

All components expose metrics on port 9090:

```bash
kubectl port-forward -n satori svc/satori-event-processor 9090:9090

# View metrics at http://localhost:9090/metrics
```

## Troubleshooting

### Check Pod Status

```bash
kubectl get pods -n satori -l app.kubernetes.io/name=satori
```

### View Logs

```bash
# Event processor logs
kubectl logs -n satori -l app.kubernetes.io/component=event-processor

# Specific camera agent logs
kubectl logs -n satori -l satori.io/camera=front-door

# Archiver logs
kubectl logs -n satori -l satori.io/archiver=s3-primary
```

### Inspect Configuration

```bash
# View agent configuration
kubectl get configmap -n satori satori-agent-front-door -o yaml

# View event processor configuration
kubectl get configmap -n satori satori-event-processor -o yaml
```

### Check Storage

```bash
# List PVCs
kubectl get pvc -n satori

# Check PVC details
kubectl describe pvc -n satori
```

### FFmpeg Issues

If a camera agent is having issues with FFmpeg:

1. Check the logs for error messages
2. Verify the RTSP URL is accessible from the pod
3. Adjust FFmpeg input arguments if needed
4. Check resource limits (CPU/memory)

```bash
# View detailed pod events
kubectl describe pod -n satori satori-agent-front-door-0

# Test RTSP connectivity from within the pod
kubectl exec -n satori satori-agent-front-door-0 -- \
  ffmpeg -rtsp_transport tcp -i rtsp://camera/stream -t 5 -f null -
```

## Upgrading

To upgrade the deployment with new values:

```bash
helm upgrade satori ./helm/satori \
  --namespace=satori \
  -f my-values.yaml
```

To upgrade to a new image version:

```bash
helm upgrade satori ./helm/satori \
  --namespace=satori \
  --set image.tag=v1.0.0
```

## Uninstalling

To remove the deployment:

```bash
helm uninstall satori --namespace=satori
```

**Note**: PersistentVolumeClaims are not automatically deleted. To remove them:

```bash
kubectl delete pvc -n satori -l app.kubernetes.io/name=satori
```

## Advanced Configuration

### Using Node Selectors

Pin specific cameras to specific nodes:

```yaml
cameras:
  - name: front-door
    url: "rtsp://camera/stream"
    nodeSelector:
      camera-zone: front
```

### Using Affinity Rules

Keep certain cameras on the same node:

```yaml
cameras:
  - name: camera-1
    affinity:
      podAffinity:
        requiredDuringSchedulingIgnoredDuringExecution:
        - labelSelector:
            matchExpressions:
            - key: app.kubernetes.io/component
              operator: In
              values:
              - agent
          topologyKey: kubernetes.io/hostname
```

### Custom Storage Classes

Use different storage classes for different workloads:

```yaml
cameras:
  - name: high-res-camera
    storage:
      storageClassName: fast-ssd
      size: 200Gi

eventProcessor:
  storage:
    storageClassName: standard
    size: 5Gi
```

## Examples

See `helm/satori/values-example.yaml` for a complete example configuration with multiple cameras, trigger templates, and archive targets.

## Support

For issues and questions:
- GitHub Issues: https://github.com/DanNixon/satori/issues
- Documentation: https://github.com/DanNixon/satori/tree/main/docs

# Quick Start Guide

This guide will help you get Satori up and running on Kubernetes in under 10 minutes.

## Prerequisites

- A running Kubernetes cluster
- `kubectl` configured to access your cluster
- `helm` 3.0+ installed
- An MQTT broker (or follow step 1 to deploy one)

## Step 1: Deploy MQTT Broker

If you don't already have an MQTT broker, deploy Mosquitto:

```bash
kubectl create namespace satori

kubectl apply -n satori -f - <<EOF
apiVersion: v1
kind: ConfigMap
metadata:
  name: mosquitto-config
data:
  mosquitto.conf: |
    listener 1883
    allow_anonymous true
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mosquitto
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
spec:
  selector:
    app: mosquitto
  ports:
  - port: 1883
EOF
```

## Step 2: Configure Your Cameras

Create a file named `my-cameras.yaml`:

```yaml
mqtt:
  broker: "mosquitto.satori.svc.cluster.local"
  port: 1883
  username: "satori"
  password: ""

cameras:
  - name: camera-1
    url: "rtsp://YOUR_CAMERA_IP/stream"
    storage:
      size: 50Gi

eventProcessor:
  triggers:
    templates:
      motion:
        cameras:
          - camera-1
        reason: "Motion detected"
        pre: 30
        post: 60

archivers:
  - name: local
    enabled: false  # Enable if you have storage configured
```

**Important**: Replace `YOUR_CAMERA_IP` with your actual camera's IP address and RTSP path.

## Step 3: Install Satori

```bash
helm install satori ./helm/satori \
  --namespace=satori \
  --create-namespace \
  -f my-cameras.yaml
```

## Step 4: Verify Installation

Check that all pods are running:

```bash
kubectl get pods -n satori -w
```

You should see:
- `satori-agent-camera-1-0` - Running
- `satori-event-processor-*` - Running

Press `Ctrl+C` when all pods show `Running` status.

## Step 5: Access the Camera Stream

Forward the agent's HTTP port to view the live stream:

```bash
kubectl port-forward -n satori svc/satori-agent-camera-1 8000:8000
```

Open your browser to: http://localhost:8000/player

You should see a live stream from your camera!

## Step 6: Trigger an Event

In a new terminal, forward the event processor port:

```bash
kubectl port-forward -n satori svc/satori-event-processor 8080:8000
```

Trigger an event:

```bash
curl -X POST http://localhost:8080/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "id": "motion",
    "reason": "Test event"
  }'
```

Check the event processor logs:

```bash
kubectl logs -n satori -l app.kubernetes.io/component=event-processor -f
```

## Next Steps

### Add More Cameras

Edit your `my-cameras.yaml` file and add more cameras:

```yaml
cameras:
  - name: camera-1
    url: "rtsp://192.168.1.10/stream"
    storage:
      size: 50Gi
  
  - name: camera-2
    url: "rtsp://192.168.1.11/stream"
    storage:
      size: 50Gi
```

Upgrade your deployment:

```bash
helm upgrade satori ./helm/satori \
  --namespace=satori \
  -f my-cameras.yaml
```

### Configure Storage

To archive video to S3, first create credentials:

```bash
kubectl create secret generic aws-credentials \
  --namespace=satori \
  --from-literal=access-key-id=YOUR_KEY \
  --from-literal=secret-access-key=YOUR_SECRET
```

Then enable the archiver in `my-cameras.yaml`:

```yaml
archivers:
  - name: s3-archive
    enabled: true
    storage:
      kind: s3
      bucket: "my-video-archive"
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

Upgrade:

```bash
helm upgrade satori ./helm/satori \
  --namespace=satori \
  -f my-cameras.yaml
```

### Create Trigger Templates

Define different recording profiles in `my-cameras.yaml`:

```yaml
eventProcessor:
  triggers:
    templates:
      motion:
        cameras: [camera-1, camera-2]
        reason: "Motion detected"
        pre: 30
        post: 60
      
      alarm:
        cameras: [camera-1, camera-2]
        reason: "Alarm triggered"
        pre: 120
        post: 300
```

Then trigger specific templates:

```bash
curl -X POST http://localhost:8080/trigger \
  -H "Content-Type: application/json" \
  -d '{"id": "alarm", "reason": "Security alarm"}'
```

## Troubleshooting

### Camera Not Connecting

Check the agent logs:

```bash
kubectl logs -n satori satori-agent-camera-1-0
```

Common issues:
- Incorrect RTSP URL
- Network connectivity
- Camera authentication required

### No Video Segments

Check:
1. FFmpeg is running: `kubectl logs -n satori satori-agent-camera-1-0 | grep ffmpeg`
2. Disk space: `kubectl exec -n satori satori-agent-camera-1-0 -- df -h /data`
3. Camera is streaming: Test with `ffmpeg` or VLC directly

### Events Not Triggering

Check event processor logs:

```bash
kubectl logs -n satori -l app.kubernetes.io/component=event-processor
```

Verify MQTT connectivity:

```bash
kubectl exec -n satori deploy/mosquitto -- mosquitto_sub -t satori -v
```

## Getting Help

- 📚 Full Documentation: [/helm/satori/README.md](./README.md)
- 📖 Kubernetes Guide: [/docs/kubernetes-deployment.md](../../docs/kubernetes-deployment.md)
- 🐛 Report Issues: https://github.com/DanNixon/satori/issues
- 💡 Examples: [values-example.yaml](./values-example.yaml)

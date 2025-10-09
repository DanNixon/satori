# Event Processor

The event processor is responsible for:

- Receiving event triggers via HTTP
- Managing active events and their lifecycle
- Requesting video segments from cameras for triggered events
- Sending archive commands to the archiver via MQTT

## HTTP API

### Trigger Endpoint

Accepts event triggers via HTTP POST.

**Endpoint:** `POST /trigger`

**Request Format:**

```json
{
  "id": "trigger-id",
  "timestamp": "2025-10-08T19:00:00Z",  // optional
  "reason": "Motion detected",          // optional
  "cameras": ["camera1", "camera2"],    // optional
  "pre": 60,                            // optional, seconds before event
  "post": 60                            // optional, seconds after event
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "id": "motion-detection",
    "reason": "Motion detected at front door",
    "cameras": ["front-door"],
    "pre": 30,
    "post": 60
  }'
```

## Configuration

The HTTP server address can be configured via:

- Command line: `--http-server-address 127.0.0.1:8080`
- Environment variable: `HTTP_SERVER_ADDRESS=127.0.0.1:8080`
- Default: `127.0.0.1:8080`

The observability/metrics endpoint can be configured via:

- Command line: `--observability-address 127.0.0.1:9090`
- Environment variable: `OBSERVABILITY_ADDRESS=127.0.0.1:9090`
- Default: `127.0.0.1:9090`

## Configuration File

See the example configuration file in the integration tests or main repository documentation.

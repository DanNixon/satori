#!/bin/sh

# Script to start MinIO locally via Podman.
# Used for running unit tests for S3 storage provider.

podman run \
  --rm \
  --net=host \
  -e "MINIO_ACCESS_KEY=minioadmin" \
  -e "MINIO_SECRET_KEY=minioadmin" \
  docker.io/minio/minio \
  server /data

#!/bin/sh

(dockerd > /dev/null 2>&1) &

timeout=60
while [ ! -S /var/run/docker.sock ] && [ $timeout -gt 0 ]; do
  sleep 1
  timeout=$((timeout-1))
done

if [ ! -S /var/run/docker.sock ]; then
  echo "Docker daemon failed to start"
  exit 1
fi

export NODE_NAME="$HOSTNAME"

exec /usr/local/bin/r8sagt

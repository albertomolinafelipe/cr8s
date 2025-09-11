# r8s

Lightweight Kubernetes-Inspired Orchestrator in Rust

<img src="logo.png" alt="r8s logo" width="200"/>

## Build Commands

- `make build` — Build all locally
- `make docker` — Build Docker images for all components
- `make docker-[server|node]` — Build Docker image for a specific component
- `docker buildx create --name r8s-builder --use`

## Run locally

- `make compose-up`

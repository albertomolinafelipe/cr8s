CACHE   ?= 0           # 0/1 -> influences buildx cache
NODES   ?= 1           # number of r8sagt replicas
GRAFANA ?= 0           # 0/1 -> toggles grafana profile

FLAGS := $(filter --%,$(MAKECMDGOALS))
MAKECMDGOALS := $(filter-out $(FLAGS),$(MAKECMDGOALS))

ifneq ($(filter --cache,$(FLAGS)),)
  CACHE := 1
endif
ifneq ($(filter --no-cache,$(FLAGS)),)
  CACHE := 0
endif

NODES_FLAG := $(filter --nodes=%,$(FLAGS))
ifneq ($(NODES_FLAG),)
  NODES := $(patsubst --nodes=%,%,$(NODES_FLAG))
endif

ifneq ($(filter --grafana,$(FLAGS)),)
  GRAFANA := 1
endif
ifneq ($(filter --no-grafana,$(FLAGS)),)
  GRAFANA := 0
endif

# Logical component names
COMPONENTS := server node
CLI := cli

# Crate names
CRATE_server := r8scp
CRATE_node   := r8sagt
CRATE_cli    := r8sctl

# Docker image tags
docker_image_server := r8scp
docker_image_node   := r8sagt

BUILD_CACHE := .buildx-cache
CACHE_FROM := --cache-from=type=local,src=$(BUILD_CACHE)
ifeq ($(CACHE),1)
  CACHE_TO := --cache-to=type=local,dest=$(BUILD_CACHE),mode=max,oci-mediatypes=true
else
  CACHE_TO :=
endif

COMPOSE_FILE = docker/docker-compose.yml

.PHONY: all build build-% docker docker-% clean up down

all: build-cli docker

build-%:
	cargo build -p $(CRATE_$*) --release
ifneq ($*,cli)
else
	cp target/release/$(CRATE_$*) .
endif

build: build-cli build-server build-node

docker: docker-server docker-node

docker-server:
	@echo -e "\033[0;32m--- CONTROL PLANE ---\033[0m"
	DOCKER_BUILDKIT=1 docker buildx build \
		-t $(docker_image_server) \
		$(CACHE_FROM) $(CACHE_TO) \
		--load \
		-f docker/Dockerfile.server .

docker-node:
	@echo -e "\033[0;32m--- NODE AGENT ---\033[0m"
	DOCKER_BUILDKIT=1 docker buildx build \
		-t $(docker_image_node) \
		$(CACHE_FROM) $(CACHE_TO) \
		--load \
		-f docker/Dockerfile.node .

# Toggle grafana
ifeq ($(GRAFANA),1)
  COMPOSE_PROFILES := grafana
else
  COMPOSE_PROFILES :=
endif

SCALE_FLAG := $(if $(NODES),--scale r8sagt=$(NODES),)

up: down
	COMPOSE_PROFILES=$(COMPOSE_PROFILES) docker compose -f $(COMPOSE_FILE) up $(SCALE_FLAG)

down:
	docker compose -f $(COMPOSE_FILE) down -v

clean:
	cargo clean
	rm -rf $(BUILD_CACHE)


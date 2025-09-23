CACHE   ?= 0           # 0/1 -> influences buildx cache
NODES   ?= 1           # number of r8sagt replicas
GRAFANA ?= 0           # 0/1 -> toggles grafana profile
CI ?= 0

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

export DOCKER_BUILDKIT=1
COMPOSE_FILE = docker/docker-compose.yml
GIT_COMMIT := $(shell git rev-parse --short HEAD)
DATE := $(shell date -u +"%Y-%m-%dT%H:%M:%SZ")

GUM_FLAGS = --show-error --spinner=minidot --spinner.foreground 10

.PHONY: all build build-% docker docker-% clean up down

# ------------------ COMPILE LOCALLY

all: build-cli docker

build-%:
	@if [ "$(CI)" = "1" ]; then \
		cargo build -p $(CRATE_$*) --release; \
	else \
		start=$$(date +%s); \
		gum spin --title "Building $*" $(GUM_FLAGS) -- \
		cargo build -p $(CRATE_$*) --release; \
		end=$$(date +%s); \
		elapsed=$$((end - start)); \
		gum style --foreground 10 "> Built $* ($${elapsed}s)"; \
	fi
	@if [ "$*" = "cli" ]; then \
		cp target/release/$(CRATE_$*) . ; \
	fi

build: build-cli build-server build-node

# ------------------ BUILD IMAGES

docker: docker-server docker-node

docker-server:
	@start=$$(date +%s); \
	gum spin --title "Building server image" $(GUM_FLAGS) -- \
	docker buildx build \
		$(CACHE_FROM) $(CACHE_TO) \
		-t $(docker_image_server):$(GIT_COMMIT) \
		-t $(docker_image_server):latest \
		--build-arg GIT_COMMIT=$(GIT_COMMIT) \
		--build-arg BUILD_DATE=$(DATE) \
		--load \
		-f docker/Dockerfile.server .; \
	end=$$(date +%s); \
	elapsed=$$((end - start)); \
	gum style --foreground 10 "> Building server image ($${elapsed}s)"


docker-node:
	@start=$$(date +%s); \
	gum spin --title "Building node image" $(GUM_FLAGS) -- \
	docker buildx build \
		$(CACHE_FROM) $(CACHE_TO) \
		-t $(docker_image_node):$(GIT_COMMIT) \
		-t $(docker_image_node):latest \
		--build-arg GIT_COMMIT=$(GIT_COMMIT) \
		--build-arg BUILD_DATE=$(DATE) \
		--load \
		-f docker/Dockerfile.node .; \
	end=$$(date +%s); \
	elapsed=$$((end - start)); \
	gum style --foreground 10 "> Building node image ($${elapsed}s)"

# ------------------ COMPOSE UP/DOWN

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
	@cargo clean
	@rm -rf $(BUILD_CACHE)
	@docker image prune -f

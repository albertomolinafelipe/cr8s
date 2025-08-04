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

.PHONY: all build build-% docker docker-% clean

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
	@echo "===================> SERVER - DOCKER IMAGE"
	@img=$(docker_image_server); \
	comp=$(CRATE_server); \
	printf '%s\n' \
		'FROM clux/muslrust:1.87.0-stable AS builder' \
		'ARG COMPONENT' \
		'WORKDIR /app' \
		'COPY . .' \
		'RUN rustup target add x86_64-unknown-linux-musl && cargo build -p $$COMPONENT --release --target x86_64-unknown-linux-musl' \
		'' \
		'FROM scratch' \
		'ARG COMPONENT' \
		'COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/$$COMPONENT /usr/local/bin/$$COMPONENT' \
		'ENTRYPOINT ["/usr/local/bin/'"$$comp"'"]' \
	| { set -e; docker build --build-arg COMPONENT=$$comp -t $$img -f - .; }

docker-node:
	@echo "===================> NODE - DOCKER IMAGE"
	@img=$(docker_image_node); \
	comp=$(CRATE_node); \
	printf '%s\n' \
		'FROM clux/muslrust:1.87.0-stable AS builder' \
		'ARG COMPONENT' \
		'WORKDIR /app' \
		'COPY . .' \
		'RUN rustup target add x86_64-unknown-linux-musl && cargo build -p $$COMPONENT --release --target x86_64-unknown-linux-musl' \
		'' \
		'FROM docker:dind' \
		'ARG COMPONENT' \
		'COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/'"$$comp"' /usr/local/bin/'"$$comp"'' \
		'COPY entrypoint.sh /entrypoint.sh' \
		'RUN chmod +x /entrypoint.sh' \
		'ENTRYPOINT ["/entrypoint.sh"]' \
	| { set -e; docker build --build-arg COMPONENT=$$comp -t $$img -f - .; }

clean:
	cargo clean

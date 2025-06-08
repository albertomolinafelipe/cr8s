COMPONENTS := server node pod
CLI := cli

docker_image_server := r8s-server
docker_image_node   := r8s-node
docker_image_pod    := r8s-pod

.PHONY: all build docker docker-% clean

all: build docker

build:
	cargo build -p $(CLI) --release

docker:
	@for comp in $(COMPONENTS); do \
		$(MAKE) docker-$$comp; \
	done

docker-%:
	@echo "Building Docker image for $*"
	@img=$(docker_image_$*); \
	printf '%s\n' \
		'FROM rust:latest AS builder' \
		'ARG COMPONENT' \
		'WORKDIR /app' \
		'COPY . .' \
		'RUN cargo build -p $$COMPONENT --release' \
		'' \
		'FROM debian:bookworm-slim' \
		'ARG COMPONENT' \
		'COPY --from=builder /app/target/release/$$COMPONENT /usr/local/bin/$$COMPONENT' \
		'ENTRYPOINT ["/usr/local/bin/$*"]' \
	| docker build --build-arg COMPONENT=$* -t $$img -f - .

clean:
	cargo clean

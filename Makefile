COMPONENTS := server node
CLI := cli

docker_image_server := r8s-server
docker_image_node   := r8s-node

.PHONY: all build build-% docker docker-% clean

all: build docker

build-%:
	cargo build -p $* --release

build: build-server build-node build-cli
	cp target/release/r8sctl .

docker: docker-server docker-node


docker-server:
	@echo "===================> SERVER - DOCKER IMAGE"
	@img=$(docker_image_server); \
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
		'ENTRYPOINT ["/usr/local/bin/server"]' \
	| { set -e; docker build --build-arg COMPONENT=server -t $$img -f - .; }


docker-node:
	@echo "===================> NODE - DOCKER IMAGE"
	@img=$(docker_image_node); \
	printf '%s\n' \
		'FROM clux/muslrust:1.87.0-stable AS builder' \
		'ARG COMPONENT' \
		'WORKDIR /app' \
		'COPY . .' \
		'RUN rustup target add x86_64-unknown-linux-musl && cargo build -p $$COMPONENT --release --target x86_64-unknown-linux-musl' \
		'' \
		'FROM docker:dind' \
		'ARG COMPONENT' \
		'COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/$$COMPONENT /usr/local/bin/$$COMPONENT' \
		'COPY entrypoint.sh /entrypoint.sh' \
		'RUN chmod +x /entrypoint.sh' \
		'ENTRYPOINT ["/entrypoint.sh"]' \
	| { set -e; docker build --build-arg COMPONENT=node -t $$img -f - .; }

clean:
	cargo clean

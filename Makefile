COMPONENTS := server node
CLI := cli

docker_image_server := r8s-server
docker_image_node   := r8s-node

.PHONY: all build docker docker-% clean

all: build docker

build:
	cargo build -p $(CLI) --release
	cp target/release/r8sctl .

docker:
	@for comp in $(COMPONENTS); do \
		$(MAKE) docker-$$comp; \
	done

docker-%:
	@echo "Building Docker image for $*"
	@img=$(docker_image_$*); \
	printf '%s\n' \
		'FROM clux/muslrust:latest AS builder' \
		'ARG COMPONENT' \
		'WORKDIR /app' \
		'COPY . .' \
		'RUN cargo build -p $$COMPONENT --release' \
		'RUN rustup target add x86_64-unknown-linux-musl && cargo build -p $$COMPONENT --release --target x86_64-unknown-linux-musl' \
		'' \
		'FROM scratch' \
		'ARG COMPONENT' \
		'COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/$$COMPONENT /usr/local/bin/$$COMPONENT' \
		'ENTRYPOINT ["/usr/local/bin/$*"]' \
	| docker build --build-arg COMPONENT=$* -t $$img -f - .

clean:
	cargo clean

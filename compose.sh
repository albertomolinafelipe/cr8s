set -euo pipefail

# Defaults
NODES=1
GRAFANA=false
COMPOSE_FILE="docker-compose.generated.yml"

trap 'rm -f "$COMPOSE_FILE"' EXIT

# parsing
while [[ $# -gt 0 ]]; do
  case "$1" in
    -n|--nodes)
      NODES="$2"
      shift 2
      ;;
    -g|--grafana)
      GRAFANA=true
      shift
      ;;
    -*)
      echo "Unknown flag: $1"
      exit 1
      ;;
    *)
      break
      ;;
  esac
done

EXTRA_ARGS=("$@")

if ! [[ "$NODES" =~ ^[0-9]+$ ]] || [ "$NODES" -lt 1 ]; then
  echo "--nodes must be a positive integer (got: $NODES)"
  exit 1
fi

docker compose -f "$COMPOSE_FILE" down -v || true

# compose file
cat > "$COMPOSE_FILE" <<EOF
services:
  r8scp:
    image: r8scp
    container_name: r8scp
    environment:
      RUST_LOG: r8scp=trace
    ports:
      - "7620:7620"
    networks:
      - r8s-net
EOF

# agent containers
for i in $(seq 1 "$NODES"); do
  if [ "$NODES" -eq 1 ]; then
    AGENT_NAME="r8sagt"
    PORT=7621
  else
    AGENT_NAME="r8sagt-$i"
    PORT=$((7620 + i))
  fi

cat >> "$COMPOSE_FILE" <<EOF
  $AGENT_NAME:
    image: r8sagt
    container_name: $AGENT_NAME
    privileged: true
    ports:
      - "$PORT"
    environment:
      NODE_PORT: $PORT
      NODE_NAME: "$AGENT_NAME"
      R8S_SERVER_PORT: 7620
      R8S_SERVER_HOST: "r8scp"
      RUST_LOG: r8sagt=trace
    depends_on:
      - r8scp
    networks:
      - r8s-net
EOF
done

# etcd
cat >> "$COMPOSE_FILE" <<EOF
  etcd:
    image: quay.io/coreos/etcd:v3.6.1
    container_name: etcd
    command:
      - /usr/local/bin/etcd
      - --name=dev
      - --data-dir=/etcd-data
      - --listen-client-urls=http://0.0.0.0:2379
      - --advertise-client-urls=http://etcd:2379
      - --log-level=error
    ports:
      - "2379:2379"
    volumes:
      - etcd-data:/etcd-data
    networks:
      - r8s-net
EOF

# grafana if requested
if [ "$GRAFANA" = true ]; then
cat >> "$COMPOSE_FILE" <<EOF
  grafana:
    image: grafana/grafana
    container_name: grafana
    ports:
      - "3000:3000"
    volumes:
      - grafana-storage:/var/lib/grafana
      - ./grafana/provisioning:/etc/grafana/provisioning
    environment:
      - GF_INSTALL_PLUGINS=marcusolsson-json-datasource
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_LOG_MODE=console
      - GF_LOG_LEVEL=off
    depends_on:
      - r8scp
    networks:
      - r8s-net
EOF
fi

# volumes and networks
cat >> "$COMPOSE_FILE" <<EOF

volumes:
  grafana-storage:
  etcd-data:

networks:
  r8s-net:
    driver: bridge
EOF

docker compose -f "$COMPOSE_FILE" up "${EXTRA_ARGS[@]}"

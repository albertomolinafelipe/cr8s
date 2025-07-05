use futures_util::TryStreamExt;
use reqwest::Client;
use serde::de::DeserializeOwned;
use testcontainers::{
    core::IntoContainerPort, runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;
use uuid::Uuid;

pub struct TestNode {
    pub address: String,
    pub name: String,
    _container: ContainerAsync<GenericImage>,
}

pub struct TestControlPlane {
    pub address: String,
    pub name: String,
    pub network: String,
    _container: ContainerAsync<GenericImage>,
    _etcd: ContainerAsync<GenericImage>,
}

impl TestControlPlane {
    pub async fn new(
        name: String,
        network: String,
        etcd: ContainerAsync<GenericImage>,
        container: ContainerAsync<GenericImage>,
    ) -> Self {
        let stdout = container.stdout(false);
        let reader = BufReader::new(stdout);
        let lines = reader.lines();

        spawn_log_task(lines, true, None);

        let port = container
            .get_host_port_ipv4(7620)
            .await
            .expect("Failed to get port");

        let address = format!("http://127.0.0.1:{}", port);

        TestControlPlane {
            address,
            name,
            network,
            _etcd: etcd,
            _container: container,
        }
    }
}

impl TestNode {
    pub async fn new(name: String, container: ContainerAsync<GenericImage>) -> Self {
        let stdout = container.stdout(false);
        let reader = BufReader::new(stdout);
        let lines = reader.lines();

        let short_name = name.chars().take(4).collect::<String>();

        spawn_log_task(lines, false, Some(short_name));

        let host_port = container
            .get_host_port_ipv4(8081)
            .await
            .expect("Failed to get port");

        let address = format!("http://127.0.0.1:{}", host_port);

        TestNode {
            address,
            name,
            _container: container,
        }
    }
}

async fn _spawn_control_plane(scheduler: bool, drift: bool) -> TestControlPlane {
    let server_name = random_name();
    let etcd_name = random_name();
    let network = random_name();

    let etcd_addr = format!("http://{}:2379", etcd_name);
    let etcd = GenericImage::new("quay.io/coreos/etcd", "v3.6.1")
        .with_exposed_port(2379.tcp())
        .with_network(&network)
        .with_container_name(&etcd_name)
        .with_env_var("ETCD_LISTEN_CLIENT_URLS", "http://0.0.0.0:2379")
        .with_env_var("ETCD_ADVERTISE_CLIENT_URLS", &etcd_addr)
        .start()
        .await
        .expect("Failed to start etcd");

    let mut container = GenericImage::new("r8s-server", "latest")
        .with_exposed_port(7620.tcp())
        .with_wait_for(testcontainers::core::WaitFor::message_on_stdout(
            "r8s-server ready",
        ))
        .with_env_var("RUST_LOG", "server=trace")
        .with_env_var("R8S_SERVER_PORT", "7620")
        .with_env_var("ETCD_ADDR", etcd_addr);

    if scheduler {
        container = container.with_env_var("RUN_SCHEDULER", scheduler.to_string());
    }
    if drift {
        container = container.with_env_var("RUN_DRIFT", drift.to_string());
    }

    let container = container
        .with_network(&network)
        .with_container_name(&server_name)
        .start()
        .await
        .expect("Failed to start control plane");

    TestControlPlane::new(server_name, network, etcd, container).await
}

pub async fn spawn_control_plane() -> TestControlPlane {
    _spawn_control_plane(true, true).await
}

pub async fn spawn_api_server() -> TestControlPlane {
    _spawn_control_plane(false, false).await
}

pub async fn spawn_node(s: &TestControlPlane) -> TestNode {
    let name = random_name();
    let container = GenericImage::new("r8s-node", "latest")
        .with_exposed_port(8081.tcp())
        .with_wait_for(testcontainers::core::WaitFor::message_on_stdout(
            "r8s-node ready",
        ))
        .with_env_var("R8S_SERVER_HOST", s.name.clone())
        .with_env_var("R8S_SERVER_PORT", "7620")
        .with_env_var("NODE_PORT", "8081")
        .with_env_var("NODE_NAME", name.clone())
        .with_env_var("RUST_LOG", "node=trace")
        .with_container_name(random_name())
        .with_network(s.network.clone())
        .start()
        .await
        .expect("Failed to start node");

    TestNode::new(name, container).await
}

pub async fn watch_stream<T, F>(url: &str, mut handle_event: F)
where
    T: DeserializeOwned,
    F: FnMut(T) + Send + 'static,
{
    let client = Client::new();
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let byte_stream = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
            let stream_reader = StreamReader::new(byte_stream);
            let mut lines = BufReader::new(stream_reader).lines();

            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<T>(&line) {
                    Ok(event) => handle_event(event),
                    Err(_) => {}
                }
            }
        }
        Ok(_) => {}
        Err(_) => {}
    }
}

fn random_name() -> String {
    Uuid::new_v4().to_string()
}

fn spawn_log_task(
    mut lines: tokio::io::Lines<BufReader<impl tokio::io::AsyncRead + Unpin + Send + 'static>>,
    is_control_plane: bool,
    short_name: Option<String>,
) {
    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            if is_control_plane {
                let prefix = "r8s-server";
                // left-align prefix in 30 spaces, then add " | "
                println!("\x1b[38;5;208m{:<15} | \x1b[0m{}", prefix, line);
            } else {
                let name = short_name.as_deref().unwrap_or("node");
                let prefix = format!("r8s-node-{}", name);
                println!("\x1b[34m{:<15} | \x1b[0m{}", prefix, line);
            }
        }
    });
}

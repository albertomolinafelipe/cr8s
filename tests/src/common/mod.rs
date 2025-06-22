use testcontainers::{
    core::IntoContainerPort, runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt
};
use uuid::Uuid;
use reqwest::Client;
use std::time::Duration;

const NETWORK: &str = "r8s-test-network";

pub struct TestControlPlane {
    pub address: String,
    pub name: String,
    _container: ContainerAsync<GenericImage>,
}

pub struct TestNode {
    pub address: String,
    pub id: Uuid,
    _container: ContainerAsync<GenericImage>,
}

pub async fn spawn_server(network: bool) -> TestControlPlane {
    
    let name = random_name();

    let mut image = GenericImage::new("r8s-server", "latest")
        .with_exposed_port(8080.tcp())
        .with_wait_for(testcontainers::core::WaitFor::message_on_stdout("r8s-server ready"))
        .with_env_var("R8S_SERVER_PORT", "8080");

    if network {
        image = image
            .with_network(NETWORK)
            .with_container_name(&name);
    }

    let container = image
        .start()
        .await
        .expect("Failed to start control plane");


    let port = container.get_host_port_ipv4(8080)
        .await
        .expect("Failed to get port");
    
    let address = format!("http://127.0.0.1:{}", port);

    TestControlPlane {
        address,
        name,
        _container: container,
    }
}


pub async fn spawn_node(server_name: String) -> TestNode {
    let container = GenericImage::new("r8s-node", "latest")
        .with_exposed_port(8081.tcp())
        .with_wait_for(testcontainers::core::WaitFor::message_on_stdout("r8s-node ready"))
        .with_env_var("R8S_SERVER_HOST", server_name)
        .with_env_var("R8S_SERVER_PORT", "8080")
        .with_env_var("NODE_PORT", "8081")
        .with_container_name(random_name())
        .with_network(NETWORK)
        .start()
        .await
        .expect("Failed to start node");

    let host_port = container.get_host_port_ipv4(8081)
        .await
        .expect("Failed to get port");


    let client = Client::new();
    let mut node_id = Uuid::nil();
    let address = format!("http://127.0.0.1:{}", host_port);
    for _ in 0..3 {
        if let Ok(resp) = client.get(format!("{}/id", address)).send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    if let Ok(uuid) = Uuid::parse_str(text.trim()) {
                        if uuid != Uuid::nil() {
                            node_id = uuid;
                            break;
                        }
                    }
                }
            }
        } 
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    if node_id == Uuid::nil() {
        panic!("Failed to get node ID from /id after 3 attempts");
    }

    TestNode {
        id: node_id,
        address,
        _container: container,
    }
}


fn random_name() -> String {
    Uuid::new_v4().to_string()
}

use testcontainers::{
    core::IntoContainerPort, runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt
};
use std::time::Duration;

pub struct TestControlPlane {
    pub address: String,
    _container: ContainerAsync<GenericImage>,
}

pub async fn spawn_control_plane() -> TestControlPlane {

    let container = GenericImage::new("r8s-server", "latest")
        .with_exposed_port(8080.tcp())
        .with_wait_for(testcontainers::core::WaitFor::message_on_stdout("r8s-server ready"))
        .with_env_var("R8S_SERVER_PORT", "8080")
        .start()
        .await
        .expect("Failed to start control plane");

    let host_port = container.get_host_port_ipv4(8080)
        .await
        .expect("Failed to get port");
    
    let address = format!("http://127.0.0.1:{}", host_port);

    std::thread::sleep(Duration::from_secs(1));

    TestControlPlane {
        address,
        _container: container,
    }
}

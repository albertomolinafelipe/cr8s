use shared::api::{EventType, PodEvent};
use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::image::CreateImageOptions;
use futures_util::stream::TryStreamExt;



pub async fn handle_event(event: PodEvent) {
    match event.event_type {
        EventType::ADDED => {
            tracing::info!("Spawning pod: {}", event.pod.metadata.user.name);
            let _ = start_pod_container(
                &event.pod.spec.containers[0].image,
                &event.pod.metadata.user.name).await;
        }
        _ => {
            tracing::warn!("Unhandled event type: {:?}", event.event_type);
        }
    }
}



pub async fn start_pod_container(image: &str, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    
    let docker = Docker::connect_with_socket_defaults()?;
    // Pull image if not present
    let options = Some(CreateImageOptions {
        from_image: image,
        ..Default::default()
    });

    let _: Vec<_> = docker.create_image(options, None, None).try_collect().await?;

    // Create container
    let container_config = Config {
        image: Some(image),
        tty: Some(true),
        ..Default::default()
    };

    docker
        .create_container(Some(CreateContainerOptions { name, platform: None }), container_config)
        .await?;

    docker
        .start_container(name, None::<StartContainerOptions<String>>)
        .await?;

    println!("Container '{}' started with image '{}'", name, image);
    Ok(())
}


use crate::common::spawn_control_plane;

#[tokio::test]
async fn node_can_register() {
    let control_plane = spawn_control_plane().await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/nodes", control_plane.address))
        .send()
        .await
        .unwrap();

    assert!(res.status().is_success());
}

use crate::common::{spawn_control_plane, spawn_node, watch_stream};
use reqwest::StatusCode;
use shared::{
    api::{NodeEvent, NodeRegisterReq},
    models::Node,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::timeout;

/// Should return an empty list of nodes when none are registered
#[tokio::test]
async fn nodes_get_empty() {
    let s = spawn_control_plane().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/nodes", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::OK,
        "GET /nodes should return 200 OK"
    );
    let pods = res.json::<Vec<Node>>().await.unwrap();
    assert!(pods.is_empty(), "Node list should be empty");
}

/// Should register a node and then return it in GET /nodes
#[tokio::test]
async fn node_register_and_get() {
    let s = spawn_control_plane().await;
    let client = reqwest::Client::new();
    let req = NodeRegisterReq {
        port: 1000,
        name: "node".to_string(),
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "Node registration should return 201 Created"
    );

    let res = client
        .get(format!("{}/nodes", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::OK,
        "GET /nodes should return 200 OK after registration"
    );
    let nodes = res.json::<Vec<Node>>().await.unwrap();
    assert_eq!(
        nodes.len(),
        1,
        "There should be exactly one registered node"
    );
    assert_eq!(nodes[0].name, "node", "Registered node name should match");
}

/// Should stream node events when nodes register, including existing ones
#[tokio::test]
async fn node_get_watch() {
    let s = spawn_control_plane().await;
    let n1 = spawn_node(&s).await;

    let events: Arc<Mutex<Vec<NodeEvent>>> = Arc::new(Mutex::new(vec![]));
    let events_clone = events.clone();

    let url = format!("{}/nodes?watch=true", s.address);
    let watcher = tokio::spawn(async move {
        watch_stream::<NodeEvent, _>(&url, move |event| {
            events_clone.lock().unwrap().push(event);
        })
        .await;
    });

    let n2 = spawn_node(&s).await;

    let received = timeout(Duration::from_secs(3), async {
        loop {
            if events.lock().unwrap().len() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    assert!(received.is_ok(), "Did not receive both node events in time");
    let collected = events.lock().unwrap().clone();
    let names: Vec<_> = collected.iter().map(|e| e.node.name.clone()).collect();
    assert!(names.contains(&n1.name), "Missing event for first node");
    assert!(names.contains(&n2.name), "Missing event for second node");

    watcher.abort();
}

/// Should reject a second node registration with the same name
#[tokio::test]
async fn node_register_repeat_name() {
    let s = spawn_control_plane().await;

    let client = reqwest::Client::new();
    let req = NodeRegisterReq {
        port: 1000,
        name: "node".to_string(),
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "First registration should succeed"
    );

    let req = NodeRegisterReq {
        port: 1001,
        name: "node".to_string(),
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::CONFLICT,
        "Duplicate node name should return 409 Conflict"
    );
}

/// Should reject a second node registration with the same IP+port combination
#[tokio::test]
async fn node_register_repeat_addr() {
    let s = spawn_control_plane().await;

    let client = reqwest::Client::new();
    let req = NodeRegisterReq {
        port: 1000,
        name: "node".to_string(),
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "First registration should succeed"
    );

    let req = NodeRegisterReq {
        port: 1000,
        name: "new_node".to_string(),
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::CONFLICT,
        "Duplicate address should return 409 Conflict"
    );
}

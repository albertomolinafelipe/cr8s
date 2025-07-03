use std::{sync::{Arc, Mutex}, time::Duration};

use crate::common::{spawn_api_server, spawn_control_plane, spawn_node, watch_stream};
use reqwest::StatusCode;
use shared::{
    api::{CreateResponse, PodEvent, PodField, PodManifest, PodPatch}, 
    models::{ContainerSpec, PodObject, PodSpec, UserMetadata}
};
use tokio::time::timeout;

/// Should return an empty list when no pods are created
#[tokio::test]
async fn pod_get_empty() {
    let s = spawn_control_plane().await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/pods", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK, "GET /pods should return 200 OK");
    let pods = res.json::<Vec<PodObject>>().await.unwrap();
    assert!(pods.is_empty(), "Pod list should be empty initially");
}

/// Should return only pods assigned to a specific node when queried with nodeName
#[tokio::test]
async fn pod_get_query() {
    let s = spawn_control_plane().await;
    let n = spawn_node(&s).await;
    let client = reqwest::Client::new();

    let req = PodManifest {
        metadata: UserMetadata { name: "nginx-pod".to_string() },
        spec: PodSpec {
            containers: vec![ContainerSpec::new()]
        }
    };

    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED, "Pod creation should return 201 Created");
    assert!(res.json::<CreateResponse>().await.is_ok(), "Pod creation response should deserialize");

    tokio::time::sleep(Duration::from_millis(500)).await;
    let res = client
        .get(format!("{}/pods?nodeName={}", s.address, n.name))
        .json(&req)
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::OK, "GET with nodeName should return 200 OK");
    let pods = res.json::<Vec<PodObject>>().await.unwrap();
    assert!(pods.len() > 0, "Expected at least one pod for the node");
    assert_eq!(pods[0].node_name, n.name, "Pod should be assigned to the correct node");
}

/// Should stream pod events: one before assignment (nodeName="") and one after (nodeName=<node>)
#[tokio::test]
async fn pod_get_watch() {
    let s = spawn_control_plane().await;
    let n = spawn_node(&s).await;
    let client = reqwest::Client::new();

    let req = PodManifest {
        metadata: UserMetadata {
            name: "nginx-pod".to_string(),
        },
        spec: PodSpec {
            containers: vec![ContainerSpec::new()],
        },
    };

    let empty_events: Arc<Mutex<Vec<PodEvent>>> = Arc::new(Mutex::new(vec![]));
    let named_events: Arc<Mutex<Vec<PodEvent>>> = Arc::new(Mutex::new(vec![]));

    let empty_events_clone = empty_events.clone();
    let named_events_clone = named_events.clone();

    let url_empty = format!("{}/pods?nodeName=&watch=true", s.address);
    let url_named = format!("{}/pods?nodeName={}&watch=true", s.address, n.name);

    let empty_watch = tokio::spawn(async move {
        watch_stream::<PodEvent, _>(&url_empty, move |event| {
            empty_events_clone.lock().unwrap().push(event);
        })
        .await;
    });

    let named_watch = tokio::spawn(async move {
        watch_stream::<PodEvent, _>(&url_named, move |event| {
            named_events_clone.lock().unwrap().push(event);
        })
        .await;
    });

    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED, "Pod creation should return 201 Created");
    assert!(res.json::<CreateResponse>().await.is_ok(), "Pod creation response should deserialize");

    // Wait for both events
    let received = timeout(Duration::from_secs(3), async {
        loop {
            let e = empty_events.lock().unwrap();
            let n = named_events.lock().unwrap();
            if e.len() >= 1 && n.len() >= 1 {
                break;
            }
            drop(e);
            drop(n);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    assert!(received.is_ok(), "Did not receive both pod events in time");

    let empty = empty_events.lock().unwrap();
    let named = named_events.lock().unwrap();

    assert_eq!(empty.len(), 1, "Should receive 1 event on empty nodeName watch");
    assert_eq!(named.len(), 1, "Should receive 1 event on named node watch");

    assert_eq!(
        empty[0].pod.node_name,
        "",
        "First event (before assignment) should have empty node name"
    );
    assert_eq!(
        named[0].pod.node_name,
        n.name,
        "Second event (after assignment) should match node name"
    );

    empty_watch.abort();
    named_watch.abort();
}

/// Should create a pod and return it via GET /pods
#[tokio::test]
async fn pod_create_and_get() {
    let s = spawn_control_plane().await;
    let client = reqwest::Client::new();
    let req = PodManifest {
        metadata: UserMetadata { name: "nginx-pod".to_string() },
        spec: PodSpec {
            containers: vec![ContainerSpec::new()]
        }
    };
    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED, "Pod creation should return 201 Created");
    assert!(res.json::<CreateResponse>().await.is_ok(), "Pod creation response should deserialize");

    let res = client
        .get(format!("{}/pods", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK, "GET /pods should return 200 OK");
    let pods = res.json::<Vec<PodObject>>().await.unwrap();
    assert_eq!(pods.len(), 1, "Should return exactly one pod");
    assert_eq!(pods[0].metadata.user.name, "nginx-pod", "Pod name should match submitted manifest");
}

/// Should fail to create a second pod with the same name
#[tokio::test]
async fn pod_create_repeat_name() {
    let s = spawn_control_plane().await;
    let client = reqwest::Client::new();
    let req = PodManifest {
        metadata: UserMetadata { name: "nginx-pod".to_string() },
        spec: PodSpec {
            containers: vec![ContainerSpec::new()]
        }
    };
    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED, "First pod creation should succeed");
    assert!(res.json::<CreateResponse>().await.is_ok(), "First pod creation response should deserialize");

    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT, "Duplicate pod name should return 409 Conflict");
}

/// Should reject pod creation with duplicate container names
#[tokio::test]
async fn pod_create_repeat_container_name() {
    let s = spawn_control_plane().await;
    let client = reqwest::Client::new();
    let req = PodManifest {
        metadata: UserMetadata { name: "nginx-pod".to_string() },
        spec: PodSpec {
            containers: vec![ContainerSpec::new(), ContainerSpec::new()]
        }
    };
    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "Duplicate container names should return 400 Bad Request");
}

/// Try to assign a pod to a non-existing node, should get unprocessable entity
#[tokio::test]
async fn pod_assign_node_invalid_node_name() {
    let s = spawn_api_server().await;
    let client = reqwest::Client::new();
    let req = PodManifest {
        metadata: UserMetadata { name: "nginx-pod".to_string() },
        spec: PodSpec {
            containers: vec![ContainerSpec::new()]
        }
    };
    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED, "First pod creation should succeed");
    assert!(res.json::<CreateResponse>().await.is_ok(), "First pod creation response should deserialize");

    let req = PodPatch {
        value: "made up node".to_string(),
        pod_field: PodField::NodeName,
    };

    let res = client
        .patch(format!("{}/pods/{}", s.address, "nginx-pod".to_string()))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY, "Invalid node name should be 422 Unprocessable");
}


/// Try to assign a non-existing pod, should get not found
#[tokio::test]
async fn pod_assign_node_not_found() {
    let s = spawn_api_server().await;
    let n = spawn_node(&s).await;
    let client = reqwest::Client::new();

    let req = PodPatch {
        value: n.name,
        pod_field: PodField::NodeName,
    };

    let res = client
        .patch(format!("{}/pods/{}", s.address, "made up pod".to_string()))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND, "Patch to non-existing pod should be 404 Not Found");
}

/// Successfully assign a pod, then try again and get conflict
#[tokio::test]
async fn pod_assign_node_double_assign() {
    let s = spawn_api_server().await;
    let n = spawn_node(&s).await;
    let client = reqwest::Client::new();
    let req = PodManifest {
        metadata: UserMetadata { name: "nginx-pod".to_string() },
        spec: PodSpec {
            containers: vec![ContainerSpec::new()]
        }
    };
    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED, "First pod creation should succeed");
    assert!(res.json::<CreateResponse>().await.is_ok(), "First pod creation response should deserialize");

    let req = PodPatch {
        value: n.name,
        pod_field: PodField::NodeName,
    };

    let res = client
        .patch(format!("{}/pods/{}", s.address, "nginx-pod".to_string()))
        .json(&req)
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body = res.text().await.unwrap();

    println!("RESP STATUS: {}", status);
    println!("RESP BODY: {}", body);
    //assert_eq!(res.status(), StatusCode::NO_CONTENT, "Successfull patch should be 204 No Content");
    assert!(false);

    let res = client
        .patch(format!("{}/pods/{}", s.address, "nginx-pod".to_string()))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT, "Assigned to already scheduled pod should be 409 Conflict");
}

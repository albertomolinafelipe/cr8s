use crate::common::{spawn_server, spawn_node};
use reqwest::StatusCode;
use shared::{
    api::{CreateResponse, PodManifest}, 
    models::{ContainerSpec, PodObject, PodSpec, UserMetadata}
};

#[tokio::test]
async fn pod_get_empty() {
    let s = spawn_server(false).await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/pods", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let pods = res.json::<Vec<PodObject>>().await.unwrap();
    assert!(pods.is_empty());
}


#[tokio::test]
async fn pod_get_query() {
    let s = spawn_server(true).await;
    let n = spawn_node(s.name).await;
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

    assert_eq!(res.status(), StatusCode::CREATED);
    assert!(res.json::<CreateResponse>().await.is_ok());
    let res = client
        .get(format!("{}/pods?nodeId={}", s.address, n.id.to_string()))
        .json(&req)
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::OK);
    let pods = res.json::<Vec<PodObject>>().await.unwrap();
    assert_eq!(pods[0].node_id, n.id);
}



#[tokio::test]
async fn pod_create_and_get() {
    let s = spawn_server(false).await;
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

    assert_eq!(res.status(), StatusCode::CREATED);
    assert!(res.json::<CreateResponse>().await.is_ok());

    let res = client
        .get(format!("{}/pods", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let pods = res.json::<Vec<PodObject>>().await.unwrap();
    assert_eq!(pods.len(), 1);
    assert_eq!(pods[0].metadata.user.name, "nginx-pod");
}


#[tokio::test]
async fn pod_create_repeat_name() {
    let s = spawn_server(false).await;
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

    assert_eq!(res.status(), StatusCode::CREATED);
    assert!(res.json::<CreateResponse>().await.is_ok());

    let res = client
        .post(format!("{}/pods", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
}

use crate::common::spawn_server;
use reqwest::StatusCode;
use shared::{api::{
    CreateResponse, NodeRegisterReq
}, models::Node};


#[tokio::test]
async fn nodes_get_empty() {
    let s = spawn_server(false).await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/nodes", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let pods = res.json::<Vec<Node>>().await.unwrap();
    assert!(pods.is_empty());
}


#[tokio::test]
async fn node_register_and_get() {
    let s = spawn_server(false).await;
    let client = reqwest::Client::new();
    let req = NodeRegisterReq {
        port: 1000,
        name: "node".to_string()
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    assert!(res.json::<CreateResponse>().await.is_ok());

    let res = client
        .get(format!("{}/nodes", s.address))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let nodes = res.json::<Vec<Node>>().await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].name, "node");
}


#[tokio::test]
async fn node_register_repeat_name() {
    let s = spawn_server(false).await;

    // first node
    let client = reqwest::Client::new();
    let req = NodeRegisterReq {
        port: 1000,
        name: "node".to_string()
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    assert!(res.json::<CreateResponse>().await.is_ok());
    
    let req = NodeRegisterReq {
        port: 1001,
        name: "node".to_string()
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT)
}


#[tokio::test]
async fn node_register_repeat_addr() {
    let s = spawn_server(false).await;

    // first node
    let client = reqwest::Client::new();
    let req = NodeRegisterReq {
        port: 1000,
        name: "node".to_string()
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    assert!(res.json::<CreateResponse>().await.is_ok());
    
    let req = NodeRegisterReq {
        port: 1000,
        name: "new_node".to_string()
    };

    let res = client
        .post(format!("{}/nodes", s.address))
        .json(&req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT)
}

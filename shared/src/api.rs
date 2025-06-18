use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct NodeRegisterReq {
    pub port: u16,
    pub name: String,
}


#[derive(Deserialize, Debug)]
pub struct PodQueryParams {
    #[serde(rename = "nodeName")]
    pub node_name: Option<String>,
} 

// jkcoxson

use serde::Serialize;

pub fn status_packet(valid_ip: bool, registered: bool) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["validIp"] = serde_json::Value::Bool(valid_ip);
    packet["registered"] = serde_json::Value::Bool(registered);
    serde_json::to_string(&packet).unwrap()
}

pub fn upload_response(success: bool, message: &str) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    serde_json::to_string(&packet).unwrap()
}

pub fn unregister_response(success: bool, message: &str) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    serde_json::to_string(&packet).unwrap()
}

pub fn list_apps_response(success: bool, message: &str, list: Vec<String>) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    let mut apps: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    for i in list {
        apps[i.as_str()] = serde_json::Value::String(i.clone());
    }
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    packet["list"] = apps;
    serde_json::to_string(&packet).unwrap()
}

pub fn launch_response(success: bool, message: &str) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    serde_json::to_string(&packet).unwrap()
}

#[derive(Serialize)]
pub struct Version {
    pub version: String,
}

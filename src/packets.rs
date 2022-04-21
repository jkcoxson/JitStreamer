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

pub fn potential_pair_response(success: bool, message: &str, code: u16) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    packet["code"] = serde_json::Value::Number(serde_json::Number::from(code));
    serde_json::to_string(&packet).unwrap()
}

pub fn potential_follow_up_response(success: bool, message: &str) -> String {
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

pub fn list_apps_response(
    success: bool,
    message: &str,
    list: serde_json::Value,
    prefered_list: serde_json::Value,
) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    packet["list"] = list;
    packet["preferedList"] = prefered_list;
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

// jkcoxson

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::backend::Counter;

pub fn status_packet(
    valid_ip: bool,
    registered: bool,
    mounting: bool,
    mount_message: &str,
) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["validIp"] = serde_json::Value::Bool(valid_ip);
    packet["registered"] = serde_json::Value::Bool(registered);
    packet["mounting"] = serde_json::Value::Bool(mounting);
    if mount_message.is_empty() {
        packet["mountMessage"] = serde_json::Value::String(
            "JitStreamer is still mounting your DMG, check back in a few minutes.".to_string(),
        );
        packet["mountFinished"] = serde_json::Value::Bool(false)
    } else {
        packet["mountMessage"] = serde_json::Value::String(mount_message.to_string());
        packet["mountFinished"] = serde_json::Value::Bool(true)
    }
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
    preferred_list: serde_json::Value,
) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    packet["list"] = list;
    packet["preferedList"] = preferred_list; // TODO: fix this spelling in the next release
    serde_json::to_string(&packet).unwrap()
}

pub fn launch_response(success: bool, message: &str) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    serde_json::to_string(&packet).unwrap()
}

pub fn attach_response(sucess: bool, message: &str) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(sucess);
    packet["message"] = serde_json::Value::String(message.to_string());
    serde_json::to_string(&packet).unwrap()
}

pub fn census_response(counter: Counter, clients: usize, version: String) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["launched"] = serde_json::Value::Number(serde_json::Number::from(counter.launched));
    packet["attached"] = serde_json::Value::Number(serde_json::Number::from(counter.attached));
    packet["fetched"] = serde_json::Value::Number(serde_json::Number::from(counter.fetched));
    packet["netmuxd"] = serde_json::Value::Number(serde_json::Number::from(counter.netmuxd));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    packet["uptime"] =
        serde_json::Value::Number(serde_json::Number::from(now - counter.uptime.as_secs()));
    packet["clients"] = serde_json::Value::Number(serde_json::Number::from(clients));
    packet["version"] = serde_json::Value::String(version);
    serde_json::to_string(&packet).unwrap()
}

pub fn install_response(success: bool, message: &str) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["success"] = serde_json::Value::Bool(success);
    packet["message"] = serde_json::Value::String(message.to_string());
    serde_json::to_string(&packet).unwrap()
}

#[derive(Serialize)]
pub struct Version {
    pub version: String,
}

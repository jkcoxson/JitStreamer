// jkcoxson

pub fn status_packet(ip: bool, udid: bool) -> String {
    let mut packet: serde_json::Value = serde_json::Value::Object(serde_json::Map::new());
    packet["validIp"] = serde_json::Value::String(ip.to_string());
    packet["validUdid"] = serde_json::Value::String(udid.to_string());
    serde_json::to_string(&packet).unwrap()
}

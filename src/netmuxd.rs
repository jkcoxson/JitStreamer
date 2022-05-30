// jkcoxson

use plist_plus::Plist;

#[derive(Debug)]
pub struct RawPacket {
    pub size: u32,
    pub version: u32,
    pub message: u32,
    pub tag: u32,
    pub plist: Plist,
}

impl RawPacket {
    pub fn new(plist: Plist, version: u32, message: u32, tag: u32) -> RawPacket {
        let plist_bytes = plist.to_string();
        let plist_bytes = plist_bytes.as_bytes();
        let size = plist_bytes.len() as u32 + 16;
        RawPacket {
            size,
            version,
            message,
            tag,
            plist,
        }
    }
}

impl From<RawPacket> for Vec<u8> {
    fn from(raw_packet: RawPacket) -> Vec<u8> {
        let mut packet = vec![];
        packet.extend_from_slice(&raw_packet.size.to_le_bytes());
        packet.extend_from_slice(&raw_packet.version.to_le_bytes());
        packet.extend_from_slice(&raw_packet.message.to_le_bytes());
        packet.extend_from_slice(&raw_packet.tag.to_le_bytes());
        packet.extend_from_slice(raw_packet.plist.to_string().as_bytes());
        packet
    }
}

pub fn add_device_packet(ip: String, udid: String) -> Result<RawPacket, ()> {
    let mut plist = Plist::new_dict();
    plist.dict_set_item("MessageType", "AddDevice".into())?;
    plist.dict_set_item("ConnectionType", "Network".into())?;
    plist.dict_set_item("ServiceName", "yurmom".into())?;
    plist.dict_set_item("DeviceID", udid.into())?;
    plist.dict_set_item("IPAddress", ip.into())?;

    Ok(RawPacket::new(plist, 1, 69, 69))
}

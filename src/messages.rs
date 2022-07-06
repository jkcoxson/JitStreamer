// Jackson Coxson

pub const PAIRING_TEST: &str = r#"Device did not respond to pairing test. This means that the device rejected the connection. 
Possible causes can include an invalid pairing file, not being on a WiFi network, or WiFi sync not being enabled."#;

pub const START_INSTPROXY: &str = r#"Unable to start instproxy. This is a connection error to the device.
Possible causes can include an invalid pairing file or a misbehaving device.
Try restarting your device. If you still have this problem, unregister and re-register.
Error:"#;

pub const START_DEBUG_SERVER: &str = r#"Unable to start debug server. The device is misbehaving.
Try restarting your device. If you still have this problem, unregister and re-register."#;

pub const LOOKUP_APPS: &str = r#"Unable to lookup apps. This is a result of the device misbehaving.
Restart your device and try again.
Error:"#;

pub const MOUNTING: &str = r#"JitStreamer is currently mounting the developer disk image.
This can take up to 5 minutes, keep your device powered on and connected.
If you receive this message multiple times, please restart your device."#;

pub const DETACH: &str = r#"Unable to send the detach command to the device.
This error can be ignored if the app still launched successfully.
Otherwise, please restart your device."#;

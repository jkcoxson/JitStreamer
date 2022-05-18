# JitStreamer
A program to activate JIT across the far reaches of the internet

This program runs on a Linux server and requires the following:
- [rusty_libimobiledevice](https://github.com/jkcoxson/rusty_libimobiledevice)
- A working Rust and C dev environment

You do not need to build this software yourself, there is a public instance. 
You can find support at the [Jit Streamer Discord server](https://imgur.com/rr9xJhX)

# Building
**Note:** These are rough building instructions for Linux
- Follow the instructions on [rusty_libimobiledevice](https://github.com/jkcoxson/rusty_libimobiledevice) to build the required dependency.
- Follow the instructions on [plist_plus](https://github.com/jkcoxson/plist_plus) to build the required dependency.
- Clone the repository and run ``cargo build --release``

# Usage
- Create a config file in your currect running directory with something like this:
```json
{
    "port": 80,
    "host": "0.0.0.0",
    "static_path": "",
    "database_path": "./database.json",
    "plist_storage": "/var/lib/lockdown", // This is different depending on your OS
    "dmg_path": "/DMG",
    "altserver_path": "", 
    "allowed_ips": []
}
```
- Set up your own VPN. For speed, I recommend [Wireguard](https://github.com/Nyr/wireguard-install). 
For getting around firewalls, I recommend [OpenVPN](https://github.com/Nyr/openvpn-install). 
If you can't open a port on your router, I recommend [ZeroTier](https://my.zerotier.com).
- Run ``sudo ./target/release/jit_streamer``

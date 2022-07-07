# JitStreamer
A program to activate JIT across the far reaches of the internet

This program runs on a Linux server and requires the following:
- [rusty_libimobiledevice](https://github.com/jkcoxson/rusty_libimobiledevice)
- A working Rust and C dev environment

You do not need to build this software yourself, there is a public instance. 
You can find support at the [JitStreamer Discord server](https://imgur.com/rr9xJhX)

# Building
**Note:** These are rough building instructions for Linux
- Install the following software:
    - git
    - autoconf
    - automake
    - libtool
    - pkg-config
    - build-essential
- Clone the repository and run ``cargo build --release``

# Usage
- Run JitStreamer and it will create an initial config file. Edit it with a text editor.
- Set up your own VPN. For speed, I recommend [Wireguard](https://github.com/Nyr/wireguard-install). 
For getting around firewalls, I recommend [OpenVPN](https://github.com/Nyr/openvpn-install). 
If you can't open a port on your router, I recommend [ZeroTier](https://my.zerotier.com).
- Run ``sudo ./target/release/jit_streamer``

# Bug Reporting
- Run with the environment variable ``RUST_LOG=info`` to see debug information.

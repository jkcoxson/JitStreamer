# JitStreamer
A program to activate JIT across the far reaches of the internet

This program runs on a Linux server and requires the following:
- [rusty_libimobiledevice](https://github.com/jkcoxson/rusty_libimobiledevice)
- A working Rust and C dev environment

Example of jitstreamer working: https://imgur.com/rr9xJhX

You do not need to build this software yourself, there is a public instance. 
You can find support at the [JitStreamer Discord server](https://discord.gg/RgpFBX3Q3k)

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

## [macOS Hosting Instructions](https://github.com/jkcoxson/JitStreamer/wiki/Building-and-self-hosting-on-macOS)

# Usage
- Run JitStreamer and it will create an initial config file. Edit it with a text editor.
- Set up your own VPN. TailScale is recommended for most users as it requires minimal setup. Otherwise use options like WireGuard, OpenVPN, or ZeroTier.
- Run ``sudo ./target/release/jit_streamer``

# Bug Reporting
- Run with the environment variable ``RUST_LOG=info`` to see debug information.

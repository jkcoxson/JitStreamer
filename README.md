# JitStreamer
A program to activate JIT across the far reaches of the internet

This program runs on a Linux server and requires the following:
- [rusty_libimobiledevice](https://github.com/jkcoxson/rusty_libimobiledevice)
- A working Rust and C dev environment

You do not need to build this software yourself.

In order to use Jitstreamer on your iOS/iPadOS device you need to be connected to wifi and to the server via a vpn the jitstreamer server accepts. After you are connected you use the [shortcut](https://www.icloud.com/shortcuts/64d6dd0bbad54993a78f3691b04cca7d) to communicate to the JitStreamer instance. Then you use the pair program using jitterbugpair or using the JitStreamer pair program.

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

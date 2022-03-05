#!/bin/bash

# Get the private key from an argument
if [ -z "$1" ]; then
    echo "Usage: $0 <private key>"
    exit 1
fi
wg pubkey <<< "$1"
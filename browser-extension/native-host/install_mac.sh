#!/bin/bash
# Install Native Messaging Host for Google Chrome on macOS

DIR="$( cd "$( dirname "$0" )" && pwd )"
TARGET_DIR="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
HOST_NAME="com.bookmark.sync.client"

# Create directory if it doesn't exist
mkdir -p "$TARGET_DIR"

# Copy manifest
cp "$DIR/manifest.json" "$TARGET_DIR/$HOST_NAME.json"

echo "Native messaging host $HOST_NAME installed for Chrome."

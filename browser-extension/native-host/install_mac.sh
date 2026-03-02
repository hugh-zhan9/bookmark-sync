#!/bin/bash
# 安装 拾页 Native Messaging Host for Google Chrome on macOS

set -e
DIR="$( cd "$( dirname "$0" )" && pwd )"
TARGET_DIR="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
HOST_NAME="com.bookmark.sync.client"
BIN_PATH="$HOME/.local/bin/shiiye-native-host"
APP_IDENTIFIER="com.zhangyukun.bookmark-sync-app"
EXTENSION_ID="${EXTENSION_ID:-$1}"
MANIFEST_SRC="$DIR/manifest.json"
MANIFEST_TMP="$(mktemp -t shiiye-native-host-manifest.XXXXXX.json)"

# 安装 better-sqlite3（Native Host 依赖）
echo "Installing Node.js dependencies..."
cd "$DIR" && npm install 2>/dev/null || true

# 创建启动脚本
mkdir -p "$(dirname "$BIN_PATH")"
echo "Creating host binary at $BIN_PATH..."
cat > "$BIN_PATH" << SCRIPT
#!/bin/bash
export SHIIYE_APP_IDENTIFIER="$APP_IDENTIFIER"
exec node "$DIR/host.js"
SCRIPT
chmod +x "$BIN_PATH"

# 注册 Native Messaging Host manifest 到 Chrome
echo "Registering manifest to Chrome..."
mkdir -p "$TARGET_DIR"
if [ -n "$EXTENSION_ID" ]; then
    echo "Using extension id: $EXTENSION_ID"
    node -e "const fs=require('fs'); const p=process.argv[1]; const out=process.argv[2]; const ext=process.argv[3]; const m=JSON.parse(fs.readFileSync(p,'utf8')); m.allowed_origins=[\`chrome-extension://\${ext}/\`]; fs.writeFileSync(out, JSON.stringify(m, null, 4));" "$MANIFEST_SRC" "$MANIFEST_TMP" "$EXTENSION_ID"
else
    echo "⚠️  EXTENSION_ID 未提供，将使用 manifest.json 里默认的 allowed_origins。"
    cp "$MANIFEST_SRC" "$MANIFEST_TMP"
fi

cp "$MANIFEST_TMP" "$TARGET_DIR/$HOST_NAME.json"

# 也注册到 Edge
EDGE_DIR="$HOME/Library/Application Support/Microsoft Edge/NativeMessagingHosts"
if [ -d "$EDGE_DIR" ] || mkdir -p "$EDGE_DIR" 2>/dev/null; then
    cp "$MANIFEST_TMP" "$EDGE_DIR/$HOST_NAME.json"
    echo "Also registered for Microsoft Edge."
fi

rm -f "$MANIFEST_TMP"

echo ""
echo "✅ 拾页 Native Messaging Host installed!"
echo "   Host binary: $BIN_PATH"
echo "   Chrome manifest: $TARGET_DIR/$HOST_NAME.json"
if [ -n "$EXTENSION_ID" ]; then
    echo "   Allowed origin: chrome-extension://$EXTENSION_ID/"
fi
echo ""
echo "⚠️  Please restart Chrome for changes to take effect."
echo "⚠️  Make sure 拾页 App is running before adding bookmarks."

# Bookmark Sync App

桌面端应用（Tauri + React + TypeScript + Rust）。

## 启动开发

```bash
npm install
npm run dev
npm run tauri dev
```

## 测试与构建

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

## 打包

```bash
npm run tauri build -- --bundles app
```

产物：

`src-tauri/target/release/bundle/macos/拾页.app`

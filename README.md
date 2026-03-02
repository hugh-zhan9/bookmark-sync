# Bookmark Sync（书签同步）

一个基于 **Tauri + React + Rust** 的本地优先书签同步项目，包含桌面应用与浏览器扩展。

## 项目结构

- `bookmark-sync-app/`：桌面端应用（前端 React + TypeScript，后端 Rust）
- `browser-extension/`：浏览器扩展（监听书签事件并通过 Native Messaging 交互）
- `docs/`：需求、技术设计、开发计划与变更记录

## 核心能力

- 本地 SQLite 书签存储与查询
- 书签事件记录与重放
- 同步仓库凭据安全保存
- 手动触发同步（为后续自动同步预留扩展）

## 本地开发

在 `bookmark-sync-app/` 目录执行：

```bash
npm install
npm run dev
npm run tauri dev
```

常用命令：

```bash
npm run build            # 前端构建
npm run tauri build      # 构建桌面安装包
cargo check --manifest-path src-tauri/Cargo.toml
```

## 发布与打包（GitHub Actions）

仓库已配置 `.github/workflows/release.yml`，当推送 `v*` 标签时触发跨平台打包（macOS / Ubuntu / Windows）。

示例：

```bash
git tag v0.0.1
git push origin v0.0.1
```

- 发版前确认本地可正常启动 `npm run tauri dev`。

## 安装指南

### macOS 提示“应用已损坏，无法打开”的解决办法

由于 Github Actions 自动打包的应用程序没有经过 Apple 开发者昂贵的 `$99/年` 签名公证（Notarization），macOS 的安全机制（Gatekeeper）会默认拦截并提示文件已损坏。

**解决方法：**

1. 将下载的 `拾页.app` 拖入到系统的 **“应用程序 (Applications)”** 文件夹中。
2. 打开“终端 (Terminal)”，执行以下命令移除应用的隔离属性（可能需要输入开机密码）：

```bash
sudo xattr -rd com.apple.quarantine /Applications/拾页.app
```
*(如果是旧版本名称，则替换上面的 `拾页.app` 为 `bookmark-sync-app.app` 等实际应用名。)*

执行完毕后，双击应用即可正常畅通打开！

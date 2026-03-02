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

## 注意事项

- 不要提交真实 Token、凭据或私有仓库地址。
- 发版前确认本地可正常启动 `npm run tauri dev`。

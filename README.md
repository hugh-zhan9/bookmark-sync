# Bookmark Sync（拾页）

本项目是一个基于 **Tauri + React + Rust + SQLite** 的本地优先书签管理与同步工具。

## 当前能力

- 本地书签管理：增删改查、文件夹、标签、多文件夹归属
- URL 清洗与去重：同一 canonical URL 只保留一条核心记录
- 浏览器导入：支持手动导入、本地扫描、自动定时导入
- 事件同步：基于 `events.ndjson` 的增量同步（Pull + Replay + Push）
- Git 目录同步：使用本机已有 Git 仓库目录，不依赖 PAT/Keychain
- 自动同步策略：
  - 应用启动自动 Pull（可配置）
  - 定时事件同步（默认 5 分钟，可配置）
  - 关闭应用自动 Push（可配置）
- 外观系统：亮色/暗色/跟随系统、背景图、语义化主题样式

## 项目结构

- `src/`：React + TypeScript 前端
- `src-tauri/`：Tauri Rust 后端
- `docs/`：需求、技术设计、开发计划、AI 变更记录

## 数据源切换（SQLite / PostgreSQL）

应用支持通过本地配置文件在 SQLite 与 PostgreSQL 间切换数据源：

- 配置文件路径：`app_config_dir/config.json`
- 当 `data_source = "sqlite"` 时启用 Git 同步
- 当 `data_source = "postgres"` 时禁用 Git 同步
- 切换数据源不会迁移旧数据源数据，切换后以当前数据源为准
- PostgreSQL 连接信息明文保存在配置文件中，请注意安全

示例配置：

```json
{
  "data_source": "sqlite",
  "postgres": {
    "host": "127.0.0.1",
    "port": 5432,
    "db": "bookmark_sync",
    "user": "bookmark",
    "password": "",
    "sslmode": "prefer"
  }
}
```

## 本地开发

在仓库根目录执行：

```bash
npm install
npm run dev
npm run tauri dev
```

常用命令：

```bash
npm test
npm run build
npm run tauri build -- --bundles app
cargo test --manifest-path src-tauri/Cargo.toml
```

## 打包产物

macOS 打包后产物路径：

`src-tauri/target/release/bundle/macos/拾页.app`

## macOS 安装提示

若系统提示“应用已损坏，无法打开”，执行：

```bash
sudo xattr -rd com.apple.quarantine /Applications/拾页.app
```

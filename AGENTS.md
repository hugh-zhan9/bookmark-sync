# Repository Guidelines

## Project Structure & Module Organization

This repository has **two main parts**:
- `bookmark-sync-app/` — Tauri 桌面应用（React + TypeScript 前端 / Rust 后端）
- `docs/` — 产品需求、技术设计、开发计划与 AI 变更记录

### `bookmark-sync-app/` 内部结构

```
bookmark-sync-app/
├── src/                    # React 前端
│   ├── App.tsx             # 主界面（书签列表、文件夹树、搜索、标签）
│   ├── App.test.tsx        # Vitest 前端单元测试
│   ├── App.realtime.test.tsx  # 实时书签同步集成测试
│   └── main.tsx            # 应用入口
├── src-tauri/
│   └── src/
│       ├── lib.rs          # 所有 Tauri 命令（见下方 API 列表）
│       ├── main.rs         # Rust 程序入口
│       ├── db/             # SQLite 数据库初始化 + browser_scanner
│       │   ├── mod.rs      # init_db, creating tables
│       │   └── browser_scanner.rs  # 本机 Chrome/Safari 书签扫描
│       ├── events/         # 事件溯源核心
│       │   ├── mod.rs      # replay_events
│       │   ├── models.rs   # BookmarkPayload, SyncEvent, EventLog
│       │   ├── cleaner.rs  # URL 净化（去追踪参数）
│       │   ├── metadata.rs # 异步抓取页面 title/favicon
│       │   └── native_messaging.rs  # Native Messaging 协议 read/write
│       └── sync/           # Git 同步层
│           ├── mod.rs      # init_or_open_repo, commit_all
│           └── credentials.rs  # macOS Keychain 凭据存取
├── app-icon.png            # 无边框处理后的 AI 生成拟物图标（源文件）
└── app-icon.svg            # 矢量版图标备份
```

---

## Tauri Commands (IPC API)

All commands are registered in `lib.rs → run()`:

| 命令 | 说明 |
|---|---|
| `get_bookmarks` | 获取所有未删除的书签（含标签列表） |
| `search_bookmarks(query)` | 全文搜索（title、url、host、标签名） |
| `add_bookmark(payload)` | 添加书签，触发 URL 净化 + 异步 metadata 抓取 |
| `update_bookmark(payload)` | 更新书签字段 |
| `delete_bookmark(id)` | 软删除书签 |
| `get_folders` | 获取所有文件夹 |
| `create_folder(name, parent_id)` | 创建文件夹 |
| `delete_folder(id)` | 删除文件夹 |
| `get_bookmarks_by_folder(folder_id)` | 按文件夹过滤书签 |
| `get_tags` | 获取所有标签 |
| `add_tag_to_bookmark(bookmark_id, tag_name)` | 打标签 |
| `get_bookmarks_by_tag(tag_id)` | 按标签过滤书签 |
| `import_browser_bookmarks` | 从本机 Chrome/Safari 导入书签 |

---

## Build, Test, and Development

Run from `bookmark-sync-app/` unless noted:

```bash
npm install                 # 安装前端依赖
npm run dev                 # 启动 Vite 前端开发服务器
npm run tauri dev           # 启动桌面应用（热更新）
npm run test                # 运行 Vitest 单元测试
npm run tauri build         # 打包分发二进制（macOS dmg, Windows exe）
npm run tauri icon <file>   # 将图片转换为全平台图标资产（src-tauri/icons/）

cargo check --manifest-path src-tauri/Cargo.toml   # 快速语法检查
cargo test  --manifest-path src-tauri/Cargo.toml   # Rust 测试
```

---

## Coding Style & Conventions

- **TypeScript**: 2-space indent, `camelCase` vars/functions, `PascalCase` components/interfaces.
- **Rust**: `rustfmt` defaults (4 spaces), `snake_case` functions/modules, `CamelCase` structs/enums.
- **Event Sourcing**: 所有数据变更必须封装为 `EventLog` → `replay_events()` 落入 SQLite，禁止直接 `INSERT/UPDATE bookmarks`。
- **Tauri Commands**: 使用显式清晰的命令名，返回 `Result<T, String>` 以便前端错误处理。

---

## Testing Guidelines

- Rust 单元测试：在 `src-tauri/src/**` 内以 `#[cfg(test)] mod tests` 组织。
- 前端测试：`App.test.tsx` 和 `App.realtime.test.tsx`（Vitest）。
- 每次前端改动后至少手动验证：添加书签、搜索书签、文件夹过滤、标签打标。

---

## Commit Guidelines

- 遵循 Conventional Commits: `feat|fix|refactor|docs|test|chore(scope): 中文摘要`
- PR 需包含：变更摘要、影响模块、本地验证步骤（截图/录屏）。

---

## Security Tips

- 禁止提交真实 Token、密钥或私有仓库地址。
- Git 同步凭据必须通过 `sync::credentials` 存入系统 Keychain，禁止存入任何文件。
- 发布前验证 `.github/workflows/release.yml` 的 Node 版本（≥22）和 Ubuntu 系统库依赖。

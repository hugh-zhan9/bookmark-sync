# AI-CONTEXT

> 单一事实源（SSOT）。所有 AI 终端上下文规则统一维护于本文件。
> 更新时间：2026-03-02 19:42:05 +0800

## Source: AGENTS.md（迁移前原文）

# Repository Guidelines

## Project Structure & Module Organization

This repository has **two main parts**:
- `src/` + `src-tauri/` — Tauri 桌面应用（React + TypeScript 前端 / Rust 后端）
- `docs/` — 产品需求、技术设计、开发计划与 AI 变更记录

### 应用代码结构

```
.
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
│       ├── events/         # 事件溯源核心
│       └── sync/           # Git 同步层
└── docs/
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

Run from repository root unless noted:

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

## Source: GEMINI.md（迁移前原文）

# GEMINI.md - 拾页 (Bookmark Sync) 项目指南

## 1. 项目概览 (Project Overview)
本项目名为**拾页**，是一个本地优先 (Local-First) 的跨浏览器书签管理工具，通过 **Event Sourcing（事件溯源）** 和 **Git** 实现无冲突的分布式同步。

### 核心架构
- **桌面端 (Tauri + Rust + React)**: 负责数据持久化（SQLite）、文件夹/标签管理、Git 同步以及浏览器书签本机扫描导入。
- **同步引擎**: 采用 Event Sourcing 模式，所有写操作经由 `replay_events()` → SQLite。Git 同步器（`sync/` 模块）将变更提交至私有仓库。

---

## 2. 目录结构与关键文件

```
bookmark-sync/
├── src/
│   ├── App.tsx                      # 主界面（书签列表、文件夹树、搜索、标签）
│   ├── App.test.tsx                 # 前端单元测试（Vitest）
│   └── App.realtime.test.tsx        # 实时同步集成测试
├── src-tauri/src/
│   ├── lib.rs                       # Tauri 命令汇总入口
│   ├── db/
│   │   ├── mod.rs                   # SQLite 初始化 + 建表
│   │   └── browser_scanner.rs       # 本机 Chrome/Safari 书签扫描
│   ├── events/
│   │   ├── models.rs                # BookmarkPayload, SyncEvent, EventLog
│   │   ├── mod.rs                   # replay_events 核心逻辑
│   │   ├── cleaner.rs               # URL 净化（去 UTM 追踪参数）
│   │   ├── metadata.rs              # 异步抓取 title/favicon
│   │   └── native_messaging.rs      # Native Messaging 协议 I/O
│   └── sync/
│       ├── mod.rs                   # init_or_open_repo, commit_all
│       └── credentials.rs           # macOS Keychain 凭据存储
└── docs/
    ├── technical-design.md
    ├── dev-plan.md
    ├── requirstment.md
    └── AI_CHANGELOG.md             # AI 自动生成的变更飞行日志
```

---

## 3. 开发、构建与测试 (Building and Running)

以下命令均在仓库根目录执行：

```bash
npm install               # 安装前端依赖
npm run dev               # 启动 Vite 前端开发服务器
npm run tauri dev         # 启动完整桌面应用（热更新）
npm run test              # 运行 Vitest 单元测试
npm run tauri build       # 构建生产安装包
npm run tauri icon <file> # 将图片转换为全平台图标资产
```

---

## 4. 技术规范与约定 (Development Conventions)

### 技术栈
- **前端**: React 19, TypeScript, Tailwind CSS, Vite
- **后端**: Rust, Tauri v2, SQLite (rusqlite), Keyring
- **同步**: Git (git2-rs), Event Sourcing

### 核心逻辑约定
1. **Event Sourcing**: 所有数据变更必须封装为 `EventLog` 并通过 `replay_events()` 落入 SQLite，禁止直接执行 INSERT/UPDATE/DELETE on `bookmarks`。
2. **凭据安全**: 严禁明文存储 GitHub Token，必须通过 `sync::credentials` 使用系统 Keychain（macOS）。
3. **URL 净化**: 录入书签前通过 `events::cleaner::clean_url()` 剥离 UTM 等追踪参数。
4. **Native Messaging**: 浏览器插件与桌面端通信标识符为 `com.bookmark.sync.client`，所用协议实现在 `events/native_messaging.rs`。

### 发布流程
- 推送以 `v` 开头的标签触发 GitHub Actions 全自动跨平台构建：
  ```bash
  git tag v1.0.0
  git push origin main
  git push origin v1.0.0
  ```
- **未签名处理（macOS）**: 下载产物后需执行以下命令绕过 Gatekeeper：
  ```bash
  sudo xattr -rd com.apple.quarantine /Applications/拾页.app
  ```

---

## 5. 已完成功能 (Implemented Features)
- [x] M1: 本地 SQLite 书签存储（增删改查）
- [x] M2: 文件夹树 + 标签管理
- [x] M3: Git 同步引擎（凭据存储 + commit）
- [x] M4: URL 净化 + 异步 metadata 抓取（title/favicon）
- [x] 全文搜索（title / url / host / 标签）
- [x] 本机浏览器书签一键导入（browser_scanner）
- [x] CI/CD: GitHub Actions 自动跨平台打包（macOS + Ubuntu + Windows）

## 6. 待办事项 (TODO / Roadmap)
- [ ] Native Messaging Host 自动随扩展安装（一键完成，无需手动运行脚本）
- [ ] Git 增量同步防抖机制（避免频繁推送）
- [ ] Safari WebExtension 包装（使用 Xcode safari-web-extension-converter）
- [ ] Apple Developer 签名 + 公证，使用户无需手动解除隔离属性

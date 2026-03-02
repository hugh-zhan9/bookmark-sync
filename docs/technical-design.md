# Bookmark Sync 技术设计（当前实现版）

## 1. 目标

构建一个本地优先的书签管理工具，支持多浏览器导入、事件级增量同步、多端一致性与可维护的界面系统。

## 2. 架构总览

- 前端：React + TypeScript（`src`）
- 后端：Tauri + Rust（`src-tauri/src`）
- 本地存储：SQLite（书签实体 + 关系表 + 应用设置）
- 事件模型：Event Sourcing（`EventLog` 回放）
- 远程同步：本机 Git 仓库目录中的 `events/events.ndjson`

## 3. 数据模型

- `bookmarks`：书签主表（含 canonical_url、host、软删除标记）
- `folders` / `folder_bookmarks`：文件夹及归属关系
- `tags` / `bookmark_tags`：标签及归属关系
- `app_settings`：应用配置（自动同步、主题、背景图等）
- `applied_event_ids`：事件幂等去重

核心约束：

- 同一 canonical URL 去重
- 事件回放幂等（重复事件不会重复应用）

## 4. 同步机制

### 4.1 浏览器 -> App

- 手动导入：`import_browser_bookmarks`
- 自动导入：
  - 启动时自动导入（可配置）
  - 定时导入（可配置分钟）

### 4.2 App -> Git（事件增量）

- Pull：从 Git 仓库拉取 `events/events.ndjson`，逐行回放
- Push：将本地事件文件同步到仓库并提交推送
- 命令：
  - `sync_event_pull_only`
  - `sync_event_push_only`
  - `sync_github_incremental`（pull + push）

### 4.3 自动策略

- 启动自动 Pull（可配置）
- 定时事件同步（默认 5 分钟，可配置）
- 关闭应用自动 Push（可配置）
- 失败补偿：关闭 Push 失败会记录 pending，下次启动 Pull 后补偿 Push
- 并发控制：前端防重入 + 后端 `sync_lock` 互斥

## 5. 设置系统

统一配置保存在 `app_settings`：

- 浏览器自动同步配置
- 事件自动同步配置
- Git 仓库目录配置
- 删除是否回写浏览器配置
- 外观配置（主题模式、背景图、遮罩强度）

## 6. UI 设计实现

- 主题：`light / dark / system`
- 背景图：开启、清除、遮罩强度
- 语义化样式体系：
  - 按钮：`btn-*`
  - 导航：`nav-*`
  - 标签：`tag-*`
  - 卡片：`bookmark-*`
  - 输入与面板：`input-*` / `panel-*`

## 7. 已知边界

- Git 同步依赖用户指定目录为有效 Git 仓库
- 背景图当前以 data URL 存本地设置，体积较大时会增大数据库

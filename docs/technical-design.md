# Bookmark Sync - Technical Design Document

## 1. 项目背景与目标
本项目旨在构建一个跨设备、跨浏览器的本地优先（Local-First）书签管理与同步工具。通过提供专业的 UI 界面，实现书签的统一管理、标签化、分组（如按 Host）以及跨设备的增量同步。

## 2. 核心架构设计

为了解决不同浏览器数据互相隔离及其数据库被独占锁定的痛点，并解决 Git 直接同步可能导致的合并冲突问题，本系统采用以下架构：

### 2.1 拓扑结构
- **前端页面 (UI/View)**：基于 React/Vue 等现代框架构建的单页应用，负责专业级的书签展示与交互。
- **本地宿主 (Tauri/Rust)**：作为桌面端守护进程，负责与操作系统交互、Git 命令行封装以及 SQLite 本地数据读写，兼顾轻量与高性能。
- **数据采集 (Browser Extension)**：开发统一的跨浏览器插件（支持 Chrome/Edge/Firefox 等），用于实时监听浏览器的 `bookmarks` 变更，通过 [Native Messaging](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging) 实时推送给桌面端。
- **远程存储 (Git/Github)**：利用目标用户的私有 Github 仓库作为后端，实现异地灾备与各端数据同步。

### 2.2 数据同步与防冲突模型 (CQRS / Event Sourcing)
- **局限与挑战**：如果直接将所有书签存为一个巨大的 `bookmarks.json` 文件，多设备同时推拉极易引发 Git 的 Merge Conflict，技术门槛不可接受。
- **破局方案 (Event Sourcing)**：系统不直接同步最终状态树。相反，将用户的每一次变更（如增加、删除、修改标签）记录为一条不可变的 **原子事件日志 (Event Log)**。
  - 日志文件示例：`EVENTS/1691234567_deviceA_add_bookmark_UUID.json`。
  - Git 的推送和拉取永远只涉及 **新增独立文件**，从物理层面彻底消灭了 Git 冲突。
  - 各设备客户端拉取到新的远程日志后，在本地的 SQLite 数据库中按时间序回放（Replay），重算本地最终的书签状态树。

## 3. 核心数据模型

### 3.1 本地 SQLite Schema 概念
- `bookmarks`: 存储清洗完毕后的基础属性 (URL, Title, Description, Favicon, Host)。
- `tags` / `folders`: 存储目录树关系、标签定义。
- `bookmark_tags`: 维护书签与标签的多对多关系实体。
- `event_cursors`: 记录本地设备已成功消费/回放的 Event Log 指针。

### 3.2 唯一性与清洗规则
- **参数剥离**：录入所有 URL 时，系统内置规则库，自动剥离基于 Tracking 目的的垃圾 Query 参数（如 `utm_source`, `ref`）。
- **去重逻辑**：同一份 Canonical URL 在本系统中只保留一条核心记录。如果一个用户在浏览器 A 将该链接收入“工作”目录，在浏览器 B 收入“未读”标签，本应用会在内部执行挂载映射合并，而不是创建两份记录。

## 4. 安全与权限控制策略
- 提供给 Git Client 同步所需使用的 Github Token（Personal Access Token 或 SSH Key），绝说明文写入系统配置文件。
- 强制规定使用操作系统自带的安全机制（macOS Keychain / Windows Credential Manager）加密存储敏感鉴权凭证。

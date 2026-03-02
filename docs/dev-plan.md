# Bookmark Sync - Development Plan

本项目采用由内而外的架构，分为四大演变里程碑，建议按此路线渐进式实现以降低技术风险。

## [M1] 核心基石：本地数据建模与引擎骨架
**目标**：建立桌面端独立运行所需的框架，打通底层逻辑与 UI 展示。

- [ ] **项目脚手架搭建**
  - 初始化 Tauri 项目（后端 Rust，前端 React/Vue + TypeScript + Tailwind 或主流 UI 库）。
- [ ] **存储抽象与 SQLite 配置**
  - 使用 `rusqlite` 或 `sqlx` 初始化桌面端的本地数据库。
  - 创建核心表结构（Bookmarks, Folders, Tags, Bookmark_Tags 等关系的 Schema）。
- [ ] **同步事件引擎内核 (Event Subsystem)**
  - 设计事件日志的数据结构格式（定义 Add, Delete, Update 等 Atomic Events）。
  - 实现本地“回放（Replay）”算法骨架：从事件日志生成最终书签树状快照的过程，并妥善处理重复及冲突合并策略。
- [ ] **主 UI 初始开发**
  - 完成书签侧边栏（分组/标签树），主区域（List/Grid 书签列表）。
  - 完成纯本地视角的增删改查前端对接工作。

## [M2] 数据采集：大一统的跨浏览器输入系统
**目标**：解决数据“如何自动、准时进入本系统”的技术难点。

- [ ] **浏览器插件工程搭建**
  - 构建通用 WebExtension，并配置 webpack/vite 适配 Chrome/Edge/Firefox 双版本包。
- [ ] **实时事件流对接**
  - 核心后台脚本（Background Script）接管系统自带的书签 `onCreated`、`onRemoved`、`onChanged` 事件。
- [ ] **Native Messaging 跨端通信通道**
  - 在 Tauri 中注册 Native App 的 Manifest。
  - 结合浏览器插件与本地守护进程，实现“用户新存即触发系统入账”的全真准实时流。
- [ ] **冷启动数据接入脚本 (One-Off Initializer)**
  - 根据跨浏览器适配读取其特定的初始 `bookmarks`（如 Chrome 的 Local State Json），实现用户第一次运行软件时的自动化沉浸式接管。

## [M3] 远端联通：无冲突 Git 分布式同步节点
**目标**：打通云网隔阂并赋予应用零冲突特性的同步心智模式。

- [ ] **本地凭证安全链条**
  - 通过 `keyring` 库挂载 macOS Keychain 或 Windows Credential Manager，用于保存个人的 Github 同步 Token (PAT) 及 Repository URL。
- [ ] **Libgit2 / Git CLI 集成**
  - 使用 `git2-rs` 或者 `std::process::Command`（系统 Git）挂接底层仓库操作。
- [ ] **多设备日志融合调配器**
  - 开发定时同步及触发式同步的队列引擎，并在获取 Git Pull 返回的新日志文件后触发 M1 的日志回放逻辑（Replay）以及重渲染 UI 通知。
- [ ] **增量备份防抖机制 (Debounce/Coalesce)**
  - 降低连续保存书签时向远端触发 Commit / Push 所带来的无谓消耗。

## [M4] 交互升华：数据提纯与高级产品能力 
**目标**：使得本应用从一个冷冰冰的同步组件进化为具备智能关怀的产品。

- [ ] **URL 级深层净化器**
  - 引入 URL 解析与 Normalize 标准化组件，剔除一切类似 `utm_campaign`, `ref`, `session_id` 等无意义冗余 query 参数。
- [ ] **数据源补缺（Metadata Polyfill）**
  - 异步作业队列：识别只有 URL 但是缺乏 Title 或 Favicon 的书签对象。
  - 进行无头请求抓取页面元信息完成 Title、描述（Summary）抓取回写填补。
- [ ] **高级复合搜索系统 (Advanced Search)**
  - 集成本地的 SQLite FTS5 (全文搜索)，对 Title、Host、Tags 提供多关键字秒级过滤支持。
- [ ] **双端全流程包装与集成测试 (Packaging & QA)**
  - 利用 GitHub Actions 提供跨平台的 `.app` (macOS), `.exe`/`.msi` (Windows) 打包。
  - 对 Event Sourcing 中关键的合流、覆盖、防篡改机制编写坚固的核心单元测试。

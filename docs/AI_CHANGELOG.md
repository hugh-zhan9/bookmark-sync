## [2026-03-02 11:01] [Feature]
- **Change**: 完成书签同步应用 M1 至 M4 全部核心里程碑开发，包含 Tauri+React 框架搭建、SQLite 增量全量持久化存储、Native Messaging 与系统底层对接通道、FTS5 全文搜索与网页元信息爬虫功能。
- **Risk Analysis**: 中等风险；首次全量代码接入，涉及 SQLite 外键联表与跨线程异步 HTTP 爬虫请求
- **Risk Level**: S2（中级: 局部功能异常、可绕过但影响效率）
- **Changed Files**:
- `.github/`
- `bookmark-sync-app/`
- `browser-extension/`
- `docs/`
----------------------------------------
## [2026-03-02 11:13] [Refactor]
- **Change**: 新增仓库根README中文文档，补充开发与发版标签触发说明
- **Risk Analysis**: 仅文档变更，功能逻辑无改动；主要风险是命令说明与实际流程偏差，已依据现有release.yml编写。
- **Risk Level**: S3（低级: 轻微行为偏差或日志/可观测性影响）
- **Changed Files**:
- `README.md`
----------------------------------------
## [2026-03-02 11:16] [Bugfix]
- **Change**: 修复 Github Actions 打包报错，将 Node.js 版本要求从 18 提升至 22 以适配 Vite 引擎与 TailwindCSS。
- **Risk Analysis**: 低级风险；仅调整云端 CI/CD 工作流配置文件
- **Risk Level**: S3（低级: 轻微行为偏差或日志/可观测性影响）
- **Changed Files**:
- `.github/workflows/release.yml`
----------------------------------------

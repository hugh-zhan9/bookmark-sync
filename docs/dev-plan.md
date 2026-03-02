# Bookmark Sync 开发计划（滚动版）

## 1. 当前完成情况

### M1 本地引擎与管理能力

- [x] Tauri + React + Rust + SQLite 基础架构
- [x] 书签/文件夹/标签模型与关系管理
- [x] URL 清洗与 canonical 去重
- [x] 搜索（标题、域名、标签）

### M2 浏览器数据接入

- [x] 本地浏览器导入（手动）
- [x] 启动自动导入（可配置）
- [x] 定时自动导入（可配置）
- [x] 导入时同步文件夹关系

### M3 事件增量同步

- [x] `events.ndjson` 事件级同步
- [x] Git 目录模式（不依赖 PAT/Keychain）
- [x] 启动 Pull / 定时同步 / 关闭 Push
- [x] 幂等与互斥（`applied_event_ids` + `sync_lock`）
- [x] 失败补偿（pending push）

### M4 交互与体验

- [x] 主题系统（亮色/暗色/跟随系统）
- [x] 背景图与遮罩配置
- [x] 样式语义化（按钮、导航、卡片、输入、面板）
- [x] 标签新增交互修复（图标合并、trim 保存）

## 2. 下一阶段（建议）

- [ ] 抽离前端样式 token 到独立主题文件，降低 `App.tsx` 样式耦合
- [ ] 为同步链路补充更多端到端测试（启动、定时、关闭场景）
- [ ] 增加同步状态可视化（最近一次 pull/push 时间与结果）
- [ ] 增加冲突可观测日志页面（而非仅文件日志）

## 3. 验证基线

每次迭代至少执行：

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

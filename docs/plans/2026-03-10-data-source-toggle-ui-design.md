# 数据源切换 UI 设计

日期：2026-03-10

## 目标与范围
- 设置面板新增“数据源”小节，仅提供开关与提示，不提供 PostgreSQL 连接信息输入。
- 切换立即生效：调用后端 `set_app_config`，成功后刷新数据。
- 切换到 PostgreSQL 前先做连通性检测，失败则保持原数据源并提示错误信息。

## 非目标
- 不在 UI 中编辑 PostgreSQL 连接信息。
- 不做数据迁移或合并。

## 交互与数据流
1. 打开设置面板时，调用 `get_app_config` 获取当前数据源。
2. 用户切换“数据源”开关：弹窗确认提示“不迁移旧数据源，以新数据源为准；PostgreSQL 连接信息需在 config.json 中配置”。
3. 确认后调用 `set_app_config`：
   - 目标为 PostgreSQL 时：后端先做连通性检测（连接 + `SELECT 1`）。
   - 成功：更新状态并调用 `refreshData()`。
   - 失败：不落盘配置，回滚开关并展示错误提示。
4. 小节内附提示：PostgreSQL 模式下 Git 同步不可用。

## 错误处理
- `set_app_config` 返回错误：显示错误文本；回滚开关。
- `data_source=postgres` 但配置缺失或连通性检测失败：后端校验失败，前端提示错误信息并保持原数据源。

## 测试策略
- 前端：`App.test.tsx` 增加切换触发 `set_app_config` 的断言；模拟失败回滚。
- 后端：新增连通性检测失败时不落盘配置的测试，保留 `switch_should_reject_invalid_pg_config`。

## 相关命令
- `get_app_config`
- `set_app_config`

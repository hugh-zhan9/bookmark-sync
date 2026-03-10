# 数据源切换（SQLite / PostgreSQL）设计

日期：2026-03-10

## 目标与范围
- 在本地 SQLite 与本地 PostgreSQL 之间切换为当前数据源。
- 仅当使用 SQLite 时启用 Git 同步；使用 PostgreSQL 时禁用 Git 同步。
- 切换数据源时不迁移旧数据源数据；切换后以当前数据源为准。
- PostgreSQL 连接信息写入本地配置文件。
- 支持“从浏览器重新导入”作为数据重建方式。

## 非目标
- 不做 SQLite 与 PostgreSQL 的数据迁移或合并。
- 不提供 PostgreSQL 安装、升级、运维能力。
- 不变更现有事件溯源核心模型，只做数据源切换与适配。

## 需求摘要
- 提供数据源开关与明确的切换提示，避免用户误以为数据丢失。
- PostgreSQL 连接失败时不得切换，保持当前数据源。
- 切换后立即刷新 UI 数据。

## 架构与组件
- 数据源路由层（DataSource Router）
  - 根据配置选择 SQLite 或 PostgreSQL。
  - 对上层暴露统一接口（现有 Tauri 命令不感知底层数据库）。
- SQLite 路径
  - 维持现有 `db::init_db(app_data_dir)` 初始化与事件回放逻辑。
- PostgreSQL 路径
  - 新增 PG 初始化入口，使用连接池（如 `deadpool-postgres` / `bb8`）。
  - 建表语句与 SQLite 保持语义一致（字段/索引/约束对齐）。
- Git 同步
  - 仅在 SQLite 数据源启用，PG 数据源分支直接禁用同步入口。
- 配置文件
  - 记录数据源开关与 PostgreSQL 连接信息。
  - UI/命令修改配置时写回文件。

## 配置文件草案
- 文件位置：`app_config_dir/config.json`（由 Tauri 提供）
- 结构示例：

```json
{
  "data_source": "sqlite",
  "postgres": {
    "host": "127.0.0.1",
    "port": 5432,
    "db": "bookmark_sync",
    "user": "bookmark",
    "password": "secret",
    "sslmode": "prefer"
  }
}
```

## 数据流与切换行为
1. 启动
   - 读取配置文件；按 `data_source` 初始化对应数据库连接。
2. 读写
   - 所有读写经过数据源路由层；底层执行对应 SQL。
3. 切换
   - 切换前提示“不迁移旧数据源，以新数据源为准”。
   - 连接成功后替换数据源并刷新 UI。
   - 连接失败则保持原数据源不变。
   - 可提供“清空目标库并重新从浏览器导入”快捷入口。

## 错误处理与安全性
- PostgreSQL 连接失败：提示错误并保持当前数据源。
- 配置缺失或无效：阻止切换并提示修复方式。
- 明文密码风险：文档中明确提示配置文件含敏感信息。

## 测试策略
- 单元测试
  - 路由层在 SQLite/PG 配置下初始化与调用正确。
  - PG 配置校验与错误路径覆盖。
- 集成测试
  - SQLite 分支：现有测试保持，通过切换验证 Git 同步入口禁用。
  - PG 分支：CRUD 基本路径可用，切换后 UI 刷新正确。
- 手动验证
  - 切换提示与确认文案是否清晰。
  - PG 连接失败是否阻止切换。

## 风险与缓解
- 数据源分叉导致旧数据不可见。
  - 缓解：强提示 + 提供“重新导入”入口。
- PG 连接不稳定。
  - 缓解：连接失败不切换，错误提示明确。

## 相关决策（ADR）
- 见 `docs/adr/2026-03-10-data-source-toggle.md`。

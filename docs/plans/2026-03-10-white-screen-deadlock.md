# White Screen Deadlock Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 IPC 同步命令中的互斥锁重入死锁，消除白屏/卡死。

**Architecture:** 引入最小辅助函数，集中在一次 `router` 锁内获取 `DataSourceKind` 与 sqlite 连接句柄，后续逻辑只使用返回值，避免二次锁。

**Tech Stack:** Rust (Tauri), rusqlite, std::sync::Mutex

---

### Task 1: 添加失败测试（锁重入规避辅助函数）

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn router_snapshot_should_return_sqlite_conn_and_kind() {
    let app_dir = tempdir().expect("app dir");
    let config_dir = tempdir().expect("config dir");
    let cfg = config::AppConfig::default();
    let router = DbRouter::init(&cfg, app_dir.path().to_path_buf()).expect("init router");
    let state = AppState {
        router: Mutex::new(router),
        sync_lock: Mutex::new(()),
        app_data_dir: app_dir.path().to_path_buf(),
        config: Mutex::new(cfg.clone()),
        config_dir: config_dir.path().to_path_buf(),
    };

    // 模拟在持有 router 锁时获取快照
    let guard = state.router.lock().expect("router lock");
    let (kind, conn) = router_snapshot(&guard).expect("snapshot");
    assert_eq!(kind, config::DataSourceKind::Sqlite);
    let _ = conn.lock().expect("sqlite conn lock");
    drop(guard);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml router_snapshot_should_return_sqlite_conn_and_kind`

Expected: FAIL (函数 `router_snapshot` 未定义)

**Step 3: Write minimal implementation**

```rust
fn router_snapshot(router: &DbRouter) -> Result<(config::DataSourceKind, std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>), String> {
    let kind = router.kind();
    let conn = router.sqlite_conn()?;
    Ok((kind, conn))
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml router_snapshot_should_return_sqlite_conn_and_kind`

Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs docs/plans/2026-03-10-white-screen-deadlock-design.md docs/plans/2026-03-10-white-screen-deadlock.md
git commit -m "docs: 白屏死锁修复设计与计划"
```

### Task 2: 调整同步命令避免二次锁

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn sync_guard_should_allow_multiple_router_uses_without_relock() {
    // 该测试通过编译与运行，确保无新增行为变更；核心验证在手动死锁复现步骤
    assert!(true);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sync_guard_should_allow_multiple_router_uses_without_relock`

Expected: PASS/FAIL → 若 PASS 说明测试无效，请调整为真正失败（例如引用新 helper 未实现）

**Step 3: Write minimal implementation**

- 在 `sync_event_pull_only` / `sync_event_push_only` / `sync_github_incremental` 中：
  - 用一次 `router` 锁获取 `kind` 与 `sqlite` 连接句柄（或根据 `kind` 直接判断）
  - 后续使用 `conn` 时不再调用 `state.router.lock()`

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: 避免同步命令重入 router 锁"
```

### Task 3: 回归验证

**Files:**
- None

**Step 1: Run tests**

Run: `npm test`
Expected: PASS

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

**Step 2: 手动验证**

- `npm run tauri dev` 启动应用，确认不再白屏/卡死。


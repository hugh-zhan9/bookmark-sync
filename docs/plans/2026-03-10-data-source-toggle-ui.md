# Data Source Toggle With PG Precheck Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在设置面板新增“数据源”小节，并在切换到 PostgreSQL 前做连通性检测，失败保持原数据源；切换成功后立即刷新数据。

**Architecture:** 前端通过 `get_app_config` 读取当前数据源状态并渲染开关；切换时弹窗确认并调用 `set_app_config`。后端在 `set_app_config` 中对 PostgreSQL 进行连通性检测并在失败时保持原路由与配置不变；成功后前端刷新数据。

**Tech Stack:** React 19, TypeScript, Tauri, Rust, Vitest

---

### Task 1: 后端切换前 PostgreSQL 连通性检测与路由保护

**Files:**
- Modify: `src-tauri/src/db/router.rs`
- Modify: `src-tauri/src/db/postgres.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db/router.rs`
- Test: `src-tauri/src/db/postgres.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/db/router.rs
#[test]
fn reinit_should_keep_previous_on_pg_failure() {
    let dir = tempdir().expect("tmp dir");
    let cfg = AppConfig::default();
    let mut router = DbRouter::init(&cfg, dir.path().to_path_buf()).expect("init");

    let mut pg_cfg = cfg.clone();
    pg_cfg.data_source = DataSourceKind::Postgres;
    pg_cfg.postgres.host = "127.0.0.1".into();
    pg_cfg.postgres.port = 1;

    let err = router.reinit(&pg_cfg).unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(router.kind(), DataSourceKind::Sqlite);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reinit_should_keep_previous_on_pg_failure`
Expected: FAIL with `router.kind()` changed or missing test

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/db/router.rs
pub fn reinit(&mut self, cfg: &AppConfig) -> Result<(), String> {
    match cfg.data_source {
        DataSourceKind::Sqlite => {
            let conn = db::init_db(self.app_data_dir.clone()).map_err(|e| e.to_string())?;
            self.sqlite = Some(Arc::new(Mutex::new(conn)));
            self.pg = None;
            self.kind = DataSourceKind::Sqlite;
        }
        DataSourceKind::Postgres => {
            let pool = postgres::init_db(&cfg.postgres)?;
            self.pg = Some(pool);
            self.sqlite = None;
            self.kind = DataSourceKind::Postgres;
        }
    }
    Ok(())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reinit_should_keep_previous_on_pg_failure`
Expected: PASS

**Step 5: Write the failing test for PG connectivity check**

```rust
// src-tauri/src/db/postgres.rs
#[test]
fn test_connection_should_fail_on_bad_host() {
    let cfg = PostgresConfig {
        host: "127.0.0.1".into(),
        port: 1,
        db: "bookmark_sync".into(),
        user: "bookmark".into(),
        password: "".into(),
        sslmode: "prefer".into(),
    };
    let err = test_connection(&cfg).unwrap_err();
    assert!(!err.is_empty());
}
```

**Step 6: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_connection_should_fail_on_bad_host`
Expected: FAIL with unresolved function `test_connection`

**Step 7: Write minimal implementation**

```rust
// src-tauri/src/db/postgres.rs
pub fn test_connection(cfg: &PostgresConfig) -> Result<(), String> {
    let dsn = build_dsn(&cfg.host, cfg.port, &cfg.db, &cfg.user, &cfg.password, &cfg.sslmode);
    let mut client = postgres::Client::connect(&dsn, NoTls).map_err(|e| e.to_string())?;
    client.simple_query("SELECT 1").map_err(|e| e.to_string())?;
    Ok(())
}
```

```rust
// src-tauri/src/lib.rs
fn set_app_config(state: State<'_, AppState>, next: config::AppConfig) -> Result<(), String> {
    if next.data_source == config::DataSourceKind::Postgres {
        validate_pg_config(&next)?;
        db::postgres::test_connection(&next.postgres)?;
    }
    let mut router = state.router.lock().map_err(|e| e.to_string())?;
    router.reinit(&next)?;
    config::save(&state.config_dir, &next)?;
    *state.config.lock().map_err(|e| e.to_string())? = next;
    Ok(())
}
```

**Step 8: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_connection_should_fail_on_bad_host`
Expected: PASS

**Step 9: Commit**

```bash
git add src-tauri/src/db/router.rs src-tauri/src/db/postgres.rs src-tauri/src/lib.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "切换到 PostgreSQL 前做连通性检测" "连接失败时应保持原数据源，避免路由状态不一致" "S2" "src-tauri/src/db/router.rs,src-tauri/src/db/postgres.rs,src-tauri/src/lib.rs"
git commit -m "feat: add postgres precheck for data source switch"
```

---

### Task 2: 读取当前数据源配置并进入前端状态

**Files:**
- Modify: `src/App.tsx`
- Test: `src/App.test.tsx`

**Step 1: Write the failing test**

```ts
// src/App.test.tsx
it("打开设置时应读取数据源配置", async () => {
  const invokeMock = vi.fn((cmd: string) => {
    if (cmd === "get_app_config") {
      return Promise.resolve({
        data_source: "sqlite",
        postgres: { host: "127.0.0.1", port: 5432, db: "bookmark_sync", user: "bookmark", password: "", sslmode: "prefer" },
      });
    }
    if (cmd === "get_browser_auto_sync_settings") {
      return Promise.resolve({ startup_enabled: false, interval_enabled: false, interval_minutes: 5 });
    }
    if (cmd === "get_event_auto_sync_settings") {
      return Promise.resolve({ startup_pull_enabled: false, interval_enabled: false, interval_minutes: 5, close_push_enabled: true });
    }
    if (cmd === "get_ui_appearance_settings") {
      return Promise.resolve({ theme_mode: "system", background_enabled: false, background_image_data_url: null, background_overlay_opacity: 45 });
    }
    return Promise.resolve();
  });
  vi.mocked(invoke).mockImplementation(invokeMock as any);
  render(<App />);
  fireEvent.click(screen.getByLabelText("设置"));
  await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_app_config"));
});
```

**Step 2: Run test to verify it fails**

Run: `npm run test`
Expected: FAIL with missing `get_app_config` call

**Step 3: Write minimal implementation**

```ts
// src/App.tsx
type AppConfig = {
  data_source: "sqlite" | "postgres";
  postgres: { host: string; port: number; db: string; user: string; password: string; sslmode: string };
};

const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
const dataSource = appConfig?.data_source ?? "sqlite";

const loadAppConfig = useCallback(async () => {
  try {
    const cfg = await invoke<AppConfig>("get_app_config");
    setAppConfig(cfg);
  } catch (e) { /* 允许失败，不阻断设置面板 */ }
}, []);

useEffect(() => {
  if (!showSettings) return;
  loadAppConfig();
}, [showSettings, loadAppConfig]);
```

**Step 4: Run test to verify it passes**

Run: `npm run test`
Expected: PASS

**Step 5: Commit**

```bash
git add src/App.tsx src/App.test.tsx
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "设置面板加载数据源配置" "配置读取失败可能导致开关状态不准确" "S2" "src/App.tsx,src/App.test.tsx"
git commit -m "feat: load data source config in settings"
```

---

### Task 3: 设置面板新增“数据源”小节并实现切换逻辑

**Files:**
- Modify: `src/App.tsx`
- Test: `src/App.test.tsx`

**Step 1: Write the failing test**

```ts
// src/App.test.tsx
it("切换数据源时应调用 set_app_config 并刷新", async () => {
  const invokeMock = vi.fn((cmd: string, payload?: any) => {
    if (cmd === "get_app_config") {
      return Promise.resolve({
        data_source: "sqlite",
        postgres: { host: "127.0.0.1", port: 5432, db: "bookmark_sync", user: "bookmark", password: "", sslmode: "prefer" },
      });
    }
    if (cmd === "set_app_config") {
      return Promise.resolve();
    }
    return Promise.resolve();
  });
  vi.mocked(invoke).mockImplementation(invokeMock as any);
  vi.spyOn(window, "confirm").mockReturnValue(true);

  render(<App />);
  fireEvent.click(screen.getByLabelText("设置"));
  const toggle = await screen.findByText(/数据源：/);
  fireEvent.click(toggle);

  await waitFor(() => {
    expect(invokeMock).toHaveBeenCalledWith("set_app_config", expect.any(Object));
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npm run test`
Expected: FAIL with missing UI and handler

**Step 3: Write minimal implementation**

```tsx
// src/App.tsx (在设置面板中新增)
<div className="panel-section space-y-4">
  <label className="block text-[10px] text-neutral-500 uppercase tracking-widest font-black">数据源</label>
  <div className="flex gap-3 flex-wrap">
    <button
      onClick={async () => {
        const next = dataSource === "sqlite" ? "postgres" : "sqlite";
        const ok = window.confirm("切换后不迁移旧数据源，以新数据源为准；PostgreSQL 连接信息需在 config.json 中配置。继续吗？");
        if (!ok) return;
        if (!appConfig) return;
        try {
          const updated = { ...appConfig, data_source: next };
          await invoke("set_app_config", updated);
          setAppConfig(updated);
          await refreshData();
        } catch (e) {
          alert(e);
          setAppConfig(appConfig);
        }
      }}
      className={`btn-base ${dataSource === "postgres" ? "btn-toggle-on" : "btn-toggle-off"}`}
    >
      数据源：{dataSource === "sqlite" ? "SQLite" : "PostgreSQL"}
    </button>
    <span className="text-xs text-neutral-500">PostgreSQL 模式下 Git 同步不可用</span>
  </div>
  <p className="text-xs text-neutral-500">连接信息请在 config.json 中修改</p>
</div>
```

**Step 4: Run test to verify it passes**

Run: `npm run test`
Expected: PASS

**Step 5: Commit**

```bash
git add src/App.tsx src/App.test.tsx
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "设置面板新增数据源切换开关" "切换失败回滚逻辑遗漏可能导致状态不一致" "S2" "src/App.tsx,src/App.test.tsx"
git commit -m "feat: add data source toggle section"
```

---

## Final Verification

Run:
- `npm run test`

Expected:
- All tests pass.

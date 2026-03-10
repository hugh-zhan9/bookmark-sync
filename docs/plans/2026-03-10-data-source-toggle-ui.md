# Data Source Toggle UI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在设置面板新增“数据源”小节，提供 SQLite/PostgreSQL 开关，切换立即生效并刷新数据，失败回滚并提示。

**Architecture:** 前端通过 `get_app_config` 读取当前数据源状态，在设置面板渲染开关；切换时弹窗确认并调用 `set_app_config`，成功刷新数据，失败回滚 UI。

**Tech Stack:** React 19, TypeScript, Tauri invoke API, Vitest

---

### Task 1: 读取当前数据源配置并进入前端状态

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

### Task 2: 设置面板新增“数据源”小节并实现切换逻辑

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

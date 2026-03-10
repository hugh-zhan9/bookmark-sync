import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Bookmark } from "./components/BookmarkItem";
import { Folder, Tag, Sidebar } from "./components/Sidebar";
import { BookmarkList } from "./components/BookmarkList";
import { PreviewPane } from "./components/PreviewPane";
import { ResizableLayout } from "./components/ResizableLayout";
interface EventAutoSyncSettings {
  startup_pull_enabled: boolean;
  interval_enabled: boolean;
  interval_minutes: number;
  close_push_enabled: boolean;
}
type ThemeMode = "system" | "light" | "dark";
interface UiAppearanceSettings {
  theme_mode: ThemeMode;
  background_enabled: boolean;
  background_image_data_url: string | null;
  background_overlay_opacity: number;
}
type DataSourceKind = "sqlite" | "postgres";
interface AppConfig {
  data_source: DataSourceKind;
  postgres: {
    host: string;
    port: number;
    db: string;
    user: string;
    password: string;
    sslmode: string;
  };
}
interface BookmarkExistsResult {
  exists: boolean;
  title?: string | null;
}

function App() {
  const [selectedBookmarkId, setSelectedBookmarkId] = useState<string | null>(null);
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [selectedFolderId, setSelectedFolderId] = useState<string | null>(null);
  const [selectedTagId, setSelectedFolderTagId] = useState<string | null>(null);
  const [newUrl, setNewUrl] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showNewFolder, setShowNewFolder] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");
  const [renamingFolder, setRenamingFolder] = useState<Folder | null>(null);
  const [renameFolderName, setRenameFolderName] = useState("");
  const [editingBookmark, setEditingBookmark] = useState<Bookmark | null>(null);
  const [editingFolderText, setEditingFolderText] = useState("");
  const [editingTagsText, setEditingTagsText] = useState("");
  const [originalEditingFolderIds, setOriginalEditingFolderIds] = useState<string[]>([]);
  const [originalEditingTags, setOriginalEditingTags] = useState<string[]>([]);
  const [addingTagToId, setAddingTagToId] = useState<string | null>(null);
  const [newTagText, setNewTagText] = useState("");
  const [gitRepoDir, setGitRepoDir] = useState("");
  const [autoSyncOnStartup, setAutoSyncOnStartup] = useState(true);
  const [autoSyncIntervalEnabled, setAutoSyncIntervalEnabled] = useState(true);
  const [autoSyncIntervalMinutes, setAutoSyncIntervalMinutes] = useState(5);
  const [syncingGithub, setSyncingGithub] = useState(false);
  const [syncDeleteToBrowser, setSyncDeleteToBrowser] = useState(false);
  const [eventSyncStartupPullEnabled, setEventSyncStartupPullEnabled] = useState(true);
  const [eventSyncIntervalEnabled, setEventSyncIntervalEnabled] = useState(true);
  const [eventSyncIntervalMinutes, setEventSyncIntervalMinutes] = useState(5);
  const [eventSyncClosePushEnabled, setEventSyncClosePushEnabled] = useState(true);
  const eventSyncInFlightRef = useRef(false);
  const browserImportInFlightRef = useRef(false);
  const [themeMode, setThemeMode] = useState<ThemeMode>("system");
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">("dark");
  const [backgroundEnabled, setBackgroundEnabled] = useState(false);
  const [backgroundImageDataUrl, setBackgroundImageDataUrl] = useState<string | null>(null);
  const [backgroundOverlayOpacity, setBackgroundOverlayOpacity] = useState(45);
  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
  const backgroundFileInputRef = useRef<HTMLInputElement | null>(null);
  const dataSource = appConfig?.data_source ?? "sqlite";

  useEffect(() => { refreshData(); loadDeleteSyncSetting(); loadSyncSettings(); loadAppearanceSettings(); }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      if (searchQuery.trim()) handleSearch(searchQuery);
      else if (selectedFolderId) fetchBookmarksByFolder(selectedFolderId);
      else if (selectedTagId) fetchBookmarksByTag(selectedTagId);
      else fetchAllBookmarks();
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery, selectedFolderId, selectedTagId]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("bookmarks-updated", async () => {
      await refreshData();
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((e) => console.error(e));
    return () => {
      if (unlisten) unlisten();
    };
  }, [searchQuery, selectedFolderId, selectedTagId]);

  useEffect(() => {
    if (!autoSyncIntervalEnabled) return;
    const minutes = Math.max(1, autoSyncIntervalMinutes);
    const timer = setInterval(() => {
      importBrowserBookmarks(false);
    }, minutes * 60 * 1000);
    return () => clearInterval(timer);
  }, [autoSyncIntervalEnabled, autoSyncIntervalMinutes]);

  useEffect(() => {
    if (!eventSyncIntervalEnabled) return;
    const minutes = Math.max(1, eventSyncIntervalMinutes);
    const timer = setInterval(() => {
      runIncrementalEventSync(false);
    }, minutes * 60 * 1000);
    return () => clearInterval(timer);
  }, [eventSyncIntervalEnabled, eventSyncIntervalMinutes]);

  // 按 ESC 关闭所有模态框
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (showSettings) { setShowSettings(false); return; }
      if (addingTagToId) { setAddingTagToId(null); return; }
      if (editingBookmark) { setEditingBookmark(null); return; }
      if (showNewFolder) { setShowNewFolder(false); return; }
    };
    window.addEventListener("keydown", handleEsc);
    return () => window.removeEventListener("keydown", handleEsc);
  }, [showSettings, addingTagToId, editingBookmark, showNewFolder]);

  useEffect(() => {
    if (themeMode !== "system") {
      setResolvedTheme(themeMode);
      return;
    }
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      setResolvedTheme("dark");
      return;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => setResolvedTheme(media.matches ? "dark" : "light");
    apply();
    const listener = () => apply();
    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", listener);
      return () => media.removeEventListener("change", listener);
    }
    media.addListener(listener);
    return () => media.removeListener(listener);
  }, [themeMode]);

  const loadAppConfig = useCallback(async () => {
    try {
      const cfg = await invoke<AppConfig>("get_app_config");
      setAppConfig(cfg);
    } catch (e) {
      console.error(e);
    }
  }, []);

  useEffect(() => {
    if (!showSettings) return;
    loadAppConfig();
  }, [showSettings, loadAppConfig]);

  async function refreshData() {
    await fetchFolders();
    await fetchTags();
    if (selectedFolderId) await fetchBookmarksByFolder(selectedFolderId);
    else if (selectedTagId) await fetchBookmarksByTag(selectedTagId);
    else await fetchAllBookmarks();
  }

  async function fetchAllBookmarks() {
    try { setBookmarks(await invoke<Bookmark[]>("get_bookmarks")); } catch (e) { console.error(e); }
  }

  async function fetchFolders() {
    try { setFolders(await invoke<Folder[]>("get_folders")); } catch (e) { console.error(e); }
  }

  async function fetchTags() {
    try { setTags(await invoke<Tag[]>("get_tags")); } catch (e) { console.error(e); }
  }

  async function fetchBookmarksByFolder(folderId: string) {
    try { setBookmarks(await invoke<Bookmark[]>("get_bookmarks_by_folder", { folderId })); } catch (e) { console.error(e); }
  }

  async function fetchBookmarksByTag(tagId: string) {
    try { setBookmarks(await invoke<Bookmark[]>("get_bookmarks_by_tag", { tagId })); } catch (e) { console.error(e); }
  }

  async function handleSearch(query: string) {
    try { setBookmarks(await invoke<Bookmark[]>("search_bookmarks", { query })); } catch (e) { console.error(e); }
  }

  async function handleAddBookmark(e: React.FormEvent) {
    e.preventDefault();
    if (!newUrl) return;
    setLoading(true);
    try {
      const exists = await invoke<BookmarkExistsResult>("check_bookmark_exists", { url: newUrl });
      if (exists.exists) {
        alert(`该网址已存在：${exists.title || newUrl}`);
        return;
      }
      const payload = { id: crypto.randomUUID(), url: newUrl, title: "Loading...", host: new URL(newUrl).hostname, created_at: new Date().toISOString(), tags: [] };
      await invoke("add_bookmark", { payload });
      setNewUrl("");
      refreshData();
    } catch (e) { alert(e); } finally { setLoading(false); }
  }

  async function handleDeleteBookmark(id: string) {
    try { await invoke("delete_bookmark", { id, syncBrowserDelete: syncDeleteToBrowser }); refreshData(); } catch (e) { alert(e); }
  }

  async function handleDeleteFolder(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    try { await invoke("write_debug_log", { message: `handleDeleteFolder click id=${id}` }); } catch { }
    try {
      await invoke("write_debug_log", { message: `handleDeleteFolder invoke id=${id}` });
      await invoke("delete_folder", { id });
      if (selectedFolderId === id) {
        setSelectedFolderId(null);
      }
      refreshData();
      await invoke("write_debug_log", { message: `handleDeleteFolder success id=${id}` });
    } catch (e) {
      try { await invoke("write_debug_log", { message: `handleDeleteFolder error id=${id} err=${String(e)}` }); } catch { }
      alert(e);
    }
  }

  async function handleRenameFolder(e: React.FormEvent) {
    e.preventDefault();
    if (!renamingFolder || !renameFolderName.trim()) return;
    try {
      await invoke("rename_folder", { id: renamingFolder.id, name: renameFolderName.trim() });
      setRenamingFolder(null);
      setRenameFolderName("");
      refreshData();
    } catch (e) { alert(e); }
  }

  async function handleUpdateBookmark(e: React.FormEvent) {
    e.preventDefault();
    if (!editingBookmark) return;
    try {
      await invoke("update_bookmark", { payload: editingBookmark });

      const nextFolderNames = normalizeFolderNames(editingFolderText);
      const existingByName = new Map(folders.map((f) => [f.name.trim().toLowerCase(), f.id]));
      const missingFolderNames = nextFolderNames.filter((name) => !existingByName.has(name.toLowerCase()));

      for (const name of missingFolderNames) {
        await invoke("create_folder", { name, parentId: null });
      }

      const latestFolders = await invoke<Folder[]>("get_folders");
      setFolders(latestFolders);
      const latestByName = new Map(latestFolders.map((f) => [f.name.trim().toLowerCase(), f.id]));
      const nextFolderIds = Array.from(
        new Set(nextFolderNames.map((name) => latestByName.get(name.toLowerCase())).filter((id): id is string => !!id))
      );
      for (const folderId of originalEditingFolderIds.filter((id) => !nextFolderIds.includes(id))) {
        await invoke("remove_bookmark_from_folder", { bookmarkId: editingBookmark.id, folderId });
      }
      for (const folderId of nextFolderIds.filter((id) => !originalEditingFolderIds.includes(id))) {
        await invoke("add_bookmark_to_folder", { bookmarkId: editingBookmark.id, folderId });
      }

      const nextTags = normalizeTags(editingTagsText);
      const originalTagSet = new Set(originalEditingTags.map((t) => t.toLowerCase()));
      const nextTagSet = new Set(nextTags.map((t) => t.toLowerCase()));

      for (const tag of originalEditingTags) {
        if (!nextTagSet.has(tag.toLowerCase())) {
          await invoke("remove_tag_from_bookmark", { bookmarkId: editingBookmark.id, tagName: tag });
        }
      }
      for (const tag of nextTags) {
        if (!originalTagSet.has(tag.toLowerCase())) {
          await invoke("add_tag_to_bookmark", { bookmarkId: editingBookmark.id, tagName: tag });
        }
      }

      setEditingBookmark(null);
      setEditingFolderText("");
      setEditingTagsText("");
      setOriginalEditingFolderIds([]);
      setOriginalEditingTags([]);
      refreshData();
    } catch (e) { alert(e); }
  }

  async function handleAddTag(e: React.FormEvent) {
    e.preventDefault();
    const tagName = newTagText.trim();
    if (!addingTagToId || !tagName) return;
    try { await invoke("add_tag_to_bookmark", { bookmarkId: addingTagToId, tagName }); setAddingTagToId(null); setNewTagText(""); refreshData(); } catch (e) { alert(e); }
  }

  async function importBrowserBookmarks(showAlert: boolean) {
    if (browserImportInFlightRef.current) {
      return;
    }
    browserImportInFlightRef.current = true;
    setImporting(true);
    try {
      const count = await invoke<number>("import_browser_bookmarks");
      if (showAlert) {
        alert(`导入完成！处理了 ${count} 个项目。`);
      }
      refreshData();
    } catch (e) {
      if (showAlert) {
        alert(e);
      } else {
        console.error(e);
      }
    } finally {
      setImporting(false);
      browserImportInFlightRef.current = false;
    }
  }

  async function handleImport() {
    await importBrowserBookmarks(true);
  }

  async function handleCreateFolder(e: React.FormEvent) {
    e.preventDefault();
    if (!newFolderName) return;
    try { await invoke("create_folder", { name: newFolderName, parentId: selectedFolderId }); setNewFolderName(""); setShowNewFolder(false); refreshData(); } catch (e) { alert(e); }
  }

  async function loadDeleteSyncSetting() {
    try {
      const enabled = await invoke<boolean>("get_delete_sync_setting");
      setSyncDeleteToBrowser(enabled);
    } catch (e) {
      console.error(e);
    }
  }

  async function loadSyncSettings() {
    try {
      const settings = await invoke<{ startup_enabled: boolean; interval_enabled: boolean; interval_minutes: number }>("get_browser_auto_sync_settings");
      setAutoSyncOnStartup(settings.startup_enabled);
      setAutoSyncIntervalEnabled(settings.interval_enabled);
      setAutoSyncIntervalMinutes(settings.interval_minutes || 5);
      const repoDir = await invoke<string>("get_git_sync_repo_dir");
      setGitRepoDir(repoDir);

      if (settings.startup_enabled) {
        importBrowserBookmarks(false).catch(console.error);
      }

      const eventSettings = await invoke<EventAutoSyncSettings>("get_event_auto_sync_settings");
      setEventSyncStartupPullEnabled(eventSettings.startup_pull_enabled);
      setEventSyncIntervalEnabled(eventSettings.interval_enabled);
      setEventSyncIntervalMinutes(eventSettings.interval_minutes || 5);
      setEventSyncClosePushEnabled(eventSettings.close_push_enabled);
      if (eventSettings.startup_pull_enabled) {
        invoke("sync_event_pull_only")
          .then(() => refreshData())
          .catch(console.error);
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function loadAppearanceSettings() {
    try {
      const settings = await invoke<UiAppearanceSettings | null>("get_ui_appearance_settings");
      if (!settings) return;
      setThemeMode(settings.theme_mode || "system");
      setBackgroundEnabled(settings.background_enabled);
      setBackgroundImageDataUrl(settings.background_image_data_url || null);
      setBackgroundOverlayOpacity(Math.max(0, Math.min(90, settings.background_overlay_opacity ?? 45)));
    } catch (e) {
      console.error(e);
    }
  }

  async function saveAutoSyncSettings(
    startupEnabled: boolean,
    intervalEnabled: boolean,
    intervalMinutes: number
  ) {
    const safeMinutes = Math.max(1, intervalMinutes || 1);
    await invoke("set_browser_auto_sync_settings", {
      startupEnabled,
      intervalEnabled,
      intervalMinutes: safeMinutes,
    });
    setAutoSyncOnStartup(startupEnabled);
    setAutoSyncIntervalEnabled(intervalEnabled);
    setAutoSyncIntervalMinutes(safeMinutes);
  }

  async function saveEventSyncSettings(
    startupPullEnabled: boolean,
    intervalEnabled: boolean,
    intervalMinutes: number,
    closePushEnabled: boolean,
  ) {
    const safeMinutes = Math.max(1, intervalMinutes || 1);
    await invoke("set_event_auto_sync_settings", {
      startupPullEnabled,
      intervalEnabled,
      intervalMinutes: safeMinutes,
      closePushEnabled,
    });
    setEventSyncStartupPullEnabled(startupPullEnabled);
    setEventSyncIntervalEnabled(intervalEnabled);
    setEventSyncIntervalMinutes(safeMinutes);
    setEventSyncClosePushEnabled(closePushEnabled);
  }

  async function runIncrementalEventSync(showAlert: boolean) {
    if (eventSyncInFlightRef.current) {
      return;
    }
    eventSyncInFlightRef.current = true;
    setSyncingGithub(true);
    try {
      await invoke("sync_github_incremental");
      await refreshData();
      if (showAlert) {
        alert("事件增量同步完成");
      }
    } catch (e) {
      if (showAlert) {
        alert(e);
      } else {
        console.error(e);
      }
    } finally {
      setSyncingGithub(false);
      eventSyncInFlightRef.current = false;
    }
  }

  async function saveAppearanceSettings(
    nextThemeMode: ThemeMode,
    nextBackgroundEnabled: boolean,
    nextBackgroundImageDataUrl: string | null,
    nextBackgroundOverlayOpacity: number
  ) {
    const safeOpacity = Math.max(0, Math.min(90, nextBackgroundOverlayOpacity || 0));
    await invoke("set_ui_appearance_settings", {
      themeMode: nextThemeMode,
      backgroundEnabled: nextBackgroundEnabled,
      backgroundImageDataUrl: nextBackgroundImageDataUrl,
      backgroundOverlayOpacity: safeOpacity,
    });
    setThemeMode(nextThemeMode);
    setBackgroundEnabled(nextBackgroundEnabled);
    setBackgroundImageDataUrl(nextBackgroundImageDataUrl);
    setBackgroundOverlayOpacity(safeOpacity);
  }

  async function handleBackgroundFileSelected(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async () => {
      const dataUrl = typeof reader.result === "string" ? reader.result : null;
      if (!dataUrl) return;
      try {
        await saveAppearanceSettings(themeMode, true, dataUrl, backgroundOverlayOpacity);
      } catch (err) {
        alert(err);
      }
    };
    reader.readAsDataURL(file);
    e.target.value = "";
  }

  function nextThemeMode(current: ThemeMode): ThemeMode {
    if (current === "system") return "light";
    if (current === "light") return "dark";
    return "system";
  }

  function themeLabel(mode: ThemeMode): string {
    if (mode === "light") return "亮色";
    if (mode === "dark") return "暗色";
    return "跟随系统";
  }

  async function beginEditBookmark(bm: Bookmark) {
    setEditingBookmark(bm);
    const currentTags = bm.tags ?? [];
    setOriginalEditingTags(currentTags);
    setEditingTagsText(currentTags.join(", "));
    try {
      const folderIds = await invoke<string[]>("get_bookmark_folders", { bookmarkId: bm.id });
      setOriginalEditingFolderIds(folderIds);
      const folderNameMap = new Map(folders.map((f) => [f.id, f.name]));
      const folderNames = folderIds.map((id) => folderNameMap.get(id)).filter((name): name is string => !!name);
      setEditingFolderText(folderNames.join(", "));
    } catch (e) {
      console.error(e);
      setOriginalEditingFolderIds([]);
      setEditingFolderText("");
    }
  }

  function normalizeTags(raw: string): string[] {
    const out: string[] = [];
    for (const token of raw.split(/[,，]/)) {
      const tag = token.trim();
      if (!tag) continue;
      if (!out.some((x) => x.toLowerCase() === tag.toLowerCase())) out.push(tag);
    }
    return out;
  }

  function normalizeFolderNames(raw: string): string[] {
    const out: string[] = [];
    for (const token of raw.split(/[,，]/)) {
      const name = token.trim();
      if (!name) continue;
      if (!out.some((x) => x.toLowerCase() === name.toLowerCase())) out.push(name);
    }
    return out;
  }

  // ── 稳定回调（useCallback）────────────────────────────────────────────
  // 这些回调引用稳定后，React.memo 包裹的三个面板才真正不会在无关 state 变化时重渲染。

  const handleSelectAll = useCallback(() => {
    setSelectedFolderId(null);
    setSelectedFolderTagId(null);
    setSearchQuery("");
    setSelectedBookmarkId(null);
  }, []);

  const handleSelectFolder = useCallback((id: string) => {
    setSelectedFolderId(id);
    setSelectedFolderTagId(null);
    setSearchQuery("");
    setSelectedBookmarkId(null);
  }, []);

  const handleSelectTag = useCallback((id: string) => {
    setSelectedFolderTagId(id);
    setSelectedFolderId(null);
    setSearchQuery("");
    setSelectedBookmarkId(null);
  }, []);

  const handleNewFolder = useCallback(() => setShowNewFolder(true), []);

  const handleOpenSettings = useCallback(() => setShowSettings(true), []);

  // 注意：此处 handleRenameFolder 已被同名函数（form submit handler）占用，
  // Sidebar 需要的是「开始重命名」，用不同命名区分。
  const handleStartRenameFolder = useCallback((f: Folder) => {
    setRenamingFolder(f);
    setRenameFolderName(f.name);
  }, []);

  const handleSelectBookmark = useCallback((bm: Bookmark) => {
    setSelectedBookmarkId(bm.id);
  }, []);

  const handleAddTag_ = useCallback((id: string) => {
    setAddingTagToId(id);
  }, []);

  const handleClosePreview = useCallback(() => setSelectedBookmarkId(null), []);

  return (
    <div data-theme={resolvedTheme} className={`app-root theme-${resolvedTheme} relative flex h-screen font-sans overflow-hidden`}>
      {backgroundEnabled && backgroundImageDataUrl && (
        <div className="app-bg-image" style={{ backgroundImage: `url(${backgroundImageDataUrl})` }} />
      )}
      <div className="app-bg-overlay" style={{ opacity: backgroundEnabled && backgroundImageDataUrl ? backgroundOverlayOpacity / 100 : 0 }} />
      {/* Main Layout - 原生三栏可拖拽布局 */}
      <ResizableLayout
        sidebar={
          <Sidebar
            folders={folders}
            tags={tags}
            selectedFolderId={selectedFolderId}
            selectedTagId={selectedTagId}
            searchQuery={searchQuery}
            importing={importing}
            onSelectAll={handleSelectAll}
            onSelectFolder={handleSelectFolder}
            onSelectTag={handleSelectTag}
            onSearch={setSearchQuery}
            onImport={handleImport}
            onNewFolder={handleNewFolder}
            onDeleteFolder={handleDeleteFolder}
            onRenameFolder={handleStartRenameFolder}
            onOpenSettings={handleOpenSettings}
          />
        }
        list={
          <BookmarkList
            bookmarks={bookmarks}
            newUrl={newUrl}
            loading={loading}
            searchQuery={searchQuery}
            selectedBookmarkId={selectedBookmarkId}
            onNewUrlChange={setNewUrl}
            onSearchQueryChange={setSearchQuery}
            onAddBookmark={handleAddBookmark}
            onSelectBookmark={handleSelectBookmark}
            onAddTag={handleAddTag_}
            onEdit={beginEditBookmark}
            onDelete={handleDeleteBookmark}
          />
        }
        preview={
          <PreviewPane
            bookmark={bookmarks.find(b => b.id === selectedBookmarkId) || null}
            onClose={handleClosePreview}
          />
        }
      />

      {/* Tag Modal */}
      {addingTagToId && (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 p-6">
          <div className="panel-shell w-full max-w-sm rounded-[2.5rem] p-10">
            <h2 className="text-xl font-bold text-white mb-6 logo-text tracking-widest text-center">打标签</h2>
            <form onSubmit={handleAddTag} className="space-y-6">
              <input autoFocus placeholder="标签名称 (如: 工作, 灵感)" className="input-field input-field-center px-5 py-4"
                value={newTagText} onChange={e => setNewTagText(e.target.value)} />
              <div className="flex justify-center gap-4">
                <button type="button" onClick={() => setAddingTagToId(null)} className="px-6 py-2 text-sm text-neutral-500 hover:text-white">取消</button>
                <button aria-label="保存标签" type="submit" className="btn-base btn-neutral px-10 py-3 text-sm rounded-2xl shadow-lg">添加</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Edit Modal */}
      {editingBookmark && (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 p-6">
          <div className="panel-shell w-full max-w-xl rounded-[3rem] p-12 animate-in fade-in zoom-in duration-300">
            <h2 className="text-2xl font-bold text-white mb-8 logo-text tracking-widest">编辑书签</h2>
            <form onSubmit={handleUpdateBookmark} className="space-y-6">
              <div className="space-y-2 text-white">
                <label className="text-[10px] text-neutral-500 uppercase tracking-widest font-black ml-1">标题</label>
                <input className="input-field px-5 py-4"
                  value={editingBookmark.title || ""} onChange={e => setEditingBookmark({ ...editingBookmark, title: e.target.value })} />
              </div>
              <div className="space-y-2 text-white">
                <label className="text-[10px] text-neutral-500 uppercase tracking-widest font-black ml-1">网址</label>
                <input className="input-field px-5 py-4"
                  value={editingBookmark.url} onChange={e => setEditingBookmark({ ...editingBookmark, url: e.target.value })} />
              </div>
              <div className="space-y-2 text-white">
                <label htmlFor="edit-folders-input" className="text-[10px] text-neutral-500 uppercase tracking-widest font-black ml-1">所属文件夹（逗号分隔）</label>
                <input
                  id="edit-folders-input"
                  aria-label="所属文件夹（逗号分隔）"
                  className="input-field px-5 py-4"
                  value={editingFolderText}
                  onChange={(e) => setEditingFolderText(e.target.value)}
                  placeholder="如：工作, 灵感, 稍后读"
                />
              </div>
              <div className="space-y-2 text-white">
                <label htmlFor="edit-tags-input" className="text-[10px] text-neutral-500 uppercase tracking-widest font-black ml-1">标签（逗号分隔）</label>
                <input
                  id="edit-tags-input"
                  aria-label="标签（逗号分隔）"
                  className="input-field px-5 py-4"
                  value={editingTagsText}
                  onChange={(e) => setEditingTagsText(e.target.value)}
                  placeholder="如：工作, 重要, 稍后读"
                />
              </div>
              <div className="flex justify-end gap-4 pt-6">
                <button
                  type="button"
                  onClick={() => {
                    setEditingBookmark(null);
                    setEditingFolderText("");
                    setEditingTagsText("");
                    setOriginalEditingFolderIds([]);
                    setOriginalEditingTags([]);
                  }}
                  className="px-6 py-3 text-sm text-neutral-500 hover:text-white"
                >
                  取消
                </button>
                <button aria-label="保存书签" type="submit" className="btn-base btn-accent px-10 py-4 text-sm rounded-2xl shadow-xl">保存</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {renamingFolder && (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 p-6">
          <div className="panel-shell w-full max-w-md rounded-[3rem] p-10 animate-in fade-in zoom-in duration-300">
            <h2 className="text-2xl font-bold text-white mb-8 logo-text tracking-widest text-center">重命名文件夹</h2>
            <form onSubmit={handleRenameFolder} className="space-y-6 text-white">
              <input autoFocus placeholder="文件夹名称" className="input-field input-field-center px-5 py-4"
                value={renameFolderName} onChange={e => setRenameFolderName(e.target.value)} />
              <div className="flex justify-center gap-4">
                <button
                  type="button"
                  onClick={() => {
                    setRenamingFolder(null);
                    setRenameFolderName("");
                  }}
                  className="px-6 py-2 text-sm text-neutral-500 hover:text-white"
                >
                  取消
                </button>
                <button aria-label="保存重命名" type="submit" className="btn-base btn-neutral px-10 py-3 text-sm rounded-2xl">保存</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {showNewFolder && (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 p-6">
          <div className="panel-shell w-full max-w-md rounded-[3rem] p-10 animate-in fade-in zoom-in duration-300">
            <h2 className="text-2xl font-bold text-white mb-8 logo-text tracking-widest text-center">新文件夹</h2>
            <form onSubmit={handleCreateFolder} className="space-y-6 text-white">
              <input autoFocus placeholder="文件夹名称" className="input-field input-field-center px-5 py-4"
                value={newFolderName} onChange={e => setNewFolderName(e.target.value)} />
              <div className="flex justify-center gap-4">
                <button type="button" onClick={() => setShowNewFolder(false)} className="px-6 py-2 text-sm text-neutral-500 hover:text-white">取消</button>
                <button type="submit" className="btn-base btn-neutral px-10 py-3 text-sm rounded-2xl">创建</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {showSettings && (
        <div
          className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-start justify-center z-50 p-6 overflow-y-auto"
          onClick={() => setShowSettings(false)}
        >
          <div
            className="panel-shell w-full max-w-lg rounded-[3rem] p-12 max-h-[90vh] overflow-y-auto my-auto"
            onClick={e => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-8">
              <h2 className="text-2xl font-bold text-white logo-text tracking-widest">设置</h2>
              <button
                onClick={() => setShowSettings(false)}
                className="w-9 h-9 flex items-center justify-center rounded-xl bg-neutral-800 text-neutral-400 hover:text-white hover:bg-neutral-700 transition-colors text-lg"
                aria-label="关闭设置"
              >
                ✕
              </button>
            </div>
            <div className="space-y-6">
              <div className="panel-section space-y-4">
                <label className="block text-[10px] text-neutral-500 uppercase tracking-widest font-black">主题与背景</label>
                <div className="flex gap-3 flex-wrap">
                  <button
                    aria-label={`主题：${themeLabel(themeMode)}`}
                    onClick={async () => {
                      const next = nextThemeMode(themeMode);
                      try {
                        await saveAppearanceSettings(next, backgroundEnabled, backgroundImageDataUrl, backgroundOverlayOpacity);
                      } catch (e) { alert(e); }
                    }}
                    className="btn-base btn-neutral"
                  >
                    主题：{themeLabel(themeMode)}
                  </button>
                  <button
                    onClick={async () => {
                      const next = !backgroundEnabled;
                      try {
                        await saveAppearanceSettings(themeMode, next, backgroundImageDataUrl, backgroundOverlayOpacity);
                      } catch (e) { alert(e); }
                    }}
                    className={`btn-base ${backgroundEnabled ? "btn-toggle-on" : "btn-toggle-off"}`}
                  >
                    背景图：{backgroundEnabled ? "开启" : "关闭"}
                  </button>
                  <button
                    onClick={() => backgroundFileInputRef.current?.click()}
                    className="btn-base btn-neutral"
                  >
                    选择背景图
                  </button>
                  <button
                    onClick={async () => {
                      try {
                        await saveAppearanceSettings(themeMode, false, null, backgroundOverlayOpacity);
                      } catch (e) { alert(e); }
                    }}
                    className="btn-base btn-neutral"
                  >
                    清除背景图
                  </button>
                </div>
                <input
                  ref={backgroundFileInputRef}
                  type="file"
                  accept="image/*"
                  className="hidden"
                  onChange={handleBackgroundFileSelected}
                />
                <input
                  placeholder="背景遮罩强度（0-90）"
                  type="number"
                  min={0}
                  max={90}
                  className="input-field"
                  value={backgroundOverlayOpacity}
                  onChange={(e) => setBackgroundOverlayOpacity(Number(e.target.value))}
                  onBlur={async () => {
                    try {
                      await saveAppearanceSettings(themeMode, backgroundEnabled, backgroundImageDataUrl, backgroundOverlayOpacity);
                    } catch (e) { alert(e); }
                  }}
                />
              </div>

              <div className="panel-section space-y-4">
                <label className="block text-[10px] text-neutral-500 uppercase tracking-widest font-black">数据源</label>
                <div className="flex gap-3 flex-wrap items-center">
                  <button
                    onClick={async () => {
                      if (!appConfig) return;
                      const next = dataSource === "sqlite" ? "postgres" : "sqlite";
                      const ok = window.confirm("切换后不迁移旧数据源，以新数据源为准；PostgreSQL 连接信息需在 config.json 中配置。继续吗？");
                      if (!ok) return;
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

              <div className="panel-section space-y-4">
                <label className="block text-[10px] text-neutral-500 uppercase tracking-widest font-black">浏览器自动同步</label>
                <div className="flex gap-3 flex-wrap">
                  <button
                    onClick={async () => {
                      const next = !autoSyncOnStartup;
                      try {
                        await saveAutoSyncSettings(next, autoSyncIntervalEnabled, autoSyncIntervalMinutes);
                      } catch (e) { alert(e); }
                    }}
                    className={`btn-base ${autoSyncOnStartup ? "btn-toggle-on" : "btn-toggle-off"}`}
                  >
                    启动自动同步：{autoSyncOnStartup ? "开启" : "关闭"}
                  </button>
                  <button
                    onClick={async () => {
                      const next = !autoSyncIntervalEnabled;
                      try {
                        await saveAutoSyncSettings(autoSyncOnStartup, next, autoSyncIntervalMinutes);
                      } catch (e) { alert(e); }
                    }}
                    className={`btn-base ${autoSyncIntervalEnabled ? "btn-toggle-on" : "btn-toggle-off"}`}
                  >
                    定时自动同步：{autoSyncIntervalEnabled ? "开启" : "关闭"}
                  </button>
                </div>
                <input
                  placeholder="自动同步间隔（分钟）"
                  type="number"
                  min={1}
                  className="input-field"
                  value={autoSyncIntervalMinutes}
                  onChange={(e) => setAutoSyncIntervalMinutes(Number(e.target.value))}
                  onBlur={async () => {
                    try {
                      await saveAutoSyncSettings(autoSyncOnStartup, autoSyncIntervalEnabled, autoSyncIntervalMinutes);
                    } catch (e) { alert(e); }
                  }}
                />
              </div>

              <div className="panel-section space-y-4">
                <label className="block text-[10px] text-neutral-500 uppercase tracking-widest font-black">Git 目录事件增量同步</label>
                <input
                  placeholder="本机 Git 仓库目录（必须已 git init/clone）"
                  className="input-field"
                  value={gitRepoDir}
                  onChange={(e) => setGitRepoDir(e.target.value)}
                />
                <div className="flex gap-3">
                  <button
                    onClick={async () => {
                      const next = !syncDeleteToBrowser;
                      try {
                        await invoke("set_delete_sync_setting", { enabled: next });
                        setSyncDeleteToBrowser(next);
                      } catch (e) { alert(e); }
                    }}
                    className={`btn-base ${syncDeleteToBrowser ? "btn-toggle-on" : "btn-toggle-off"}`}
                  >
                    删除同步浏览器：{syncDeleteToBrowser ? "开启" : "关闭"}
                  </button>
                  <button
                    onClick={async () => {
                      try {
                        const branch = await invoke<string>("set_git_sync_repo_dir", { repoDir: gitRepoDir });
                        alert(`已保存 Git 仓库目录，当前分支：${branch}`);
                        setShowSettings(false);
                      } catch (e) { alert(e); }
                    }}
                    className="btn-base btn-neutral"
                  >
                    保存配置
                  </button>
                  <button
                    onClick={async () => {
                      await runIncrementalEventSync(true);
                    }}
                    className="btn-base btn-accent"
                  >
                    {syncingGithub ? "同步中..." : "立即同步"}
                  </button>
                </div>
              </div>
              <div className="panel-section space-y-4">
                <label className="block text-[10px] text-neutral-500 uppercase tracking-widest font-black">事件自动同步策略</label>
                <div className="flex gap-3 flex-wrap">
                  <button
                    onClick={async () => {
                      const next = !eventSyncStartupPullEnabled;
                      try {
                        await saveEventSyncSettings(
                          next,
                          eventSyncIntervalEnabled,
                          eventSyncIntervalMinutes,
                          eventSyncClosePushEnabled
                        );
                      } catch (e) { alert(e); }
                    }}
                    className={`btn-base ${eventSyncStartupPullEnabled ? "btn-toggle-on" : "btn-toggle-off"}`}
                  >
                    启动自动 Pull：{eventSyncStartupPullEnabled ? "开启" : "关闭"}
                  </button>
                  <button
                    onClick={async () => {
                      const next = !eventSyncIntervalEnabled;
                      try {
                        await saveEventSyncSettings(
                          eventSyncStartupPullEnabled,
                          next,
                          eventSyncIntervalMinutes,
                          eventSyncClosePushEnabled
                        );
                      } catch (e) { alert(e); }
                    }}
                    className={`btn-base ${eventSyncIntervalEnabled ? "btn-toggle-on" : "btn-toggle-off"}`}
                  >
                    定时事件同步：{eventSyncIntervalEnabled ? "开启" : "关闭"}
                  </button>
                  <button
                    onClick={async () => {
                      const next = !eventSyncClosePushEnabled;
                      try {
                        await saveEventSyncSettings(
                          eventSyncStartupPullEnabled,
                          eventSyncIntervalEnabled,
                          eventSyncIntervalMinutes,
                          next
                        );
                      } catch (e) { alert(e); }
                    }}
                    className={`btn-base ${eventSyncClosePushEnabled ? "btn-toggle-on" : "btn-toggle-off"}`}
                  >
                    关闭自动 Push：{eventSyncClosePushEnabled ? "开启" : "关闭"}
                  </button>
                </div>
                <input
                  placeholder="事件同步间隔（分钟）"
                  type="number"
                  min={1}
                  className="input-field"
                  value={eventSyncIntervalMinutes}
                  onChange={(e) => setEventSyncIntervalMinutes(Number(e.target.value))}
                  onBlur={async () => {
                    try {
                      await saveEventSyncSettings(
                        eventSyncStartupPullEnabled,
                        eventSyncIntervalEnabled,
                        eventSyncIntervalMinutes,
                        eventSyncClosePushEnabled
                      );
                    } catch (e) { alert(e); }
                  }}
                />
              </div>
              <div className="flex justify-end pt-4">
                <button onClick={() => setShowSettings(false)} className="btn-base btn-neutral px-10 py-3 text-sm rounded-2xl">关闭</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;

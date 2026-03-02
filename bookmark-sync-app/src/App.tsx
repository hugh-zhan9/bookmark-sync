import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import "./App.css";

interface Bookmark {
  id: string;
  url: string;
  title?: string;
  description?: string;
  favicon_url?: string;
  host?: string;
  created_at: string;
}

function App() {
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [newUrl, setNewUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [repoUrl, setRepoUrl] = useState("");
  const [token, setToken] = useState("");
  const [syncing, setSyncing] = useState(false);

  const [searchQuery, setSearchQuery] = useState("");

  useEffect(() => {
    fetchBookmarks("");
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      fetchBookmarks(searchQuery);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let disposed = false;

    listen("bookmarks-updated", () => {
      fetchBookmarks(searchQuery);
    })
      .then((fn) => {
        if (disposed) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((error) => {
        console.error("Failed to subscribe bookmarks-updated:", error);
      });

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [searchQuery]);

  async function fetchBookmarks(query = searchQuery) {
    try {
      let data: Bookmark[] = [];
      if (query.trim() === "") {
        data = await invoke("get_bookmarks");
      } else {
        data = await invoke("search_bookmarks", { query });
      }
      setBookmarks(data);
    } catch (error) {
      console.error("Failed to fetch bookmarks:", error);
    }
  }

  async function handleAddBookmark(e: React.FormEvent) {
    e.preventDefault();
    if (!newUrl) return;
    setLoading(true);

    try {
      const host = new URL(newUrl).hostname;

      const payload: Bookmark = {
        id: crypto.randomUUID(), // For M1, generate local UUID
        url: newUrl,
        title: `Mock Title for ${host}`,
        host,
        created_at: new Date().toISOString(),
      };

      await invoke("add_bookmark", { payload });
      setNewUrl("");
      await fetchBookmarks(); // Refresh list after adding
    } catch (error) {
      console.error("Failed to add bookmark:", error);
      alert("Error adding bookmark! Is it a valid URL?");
    } finally {
      setLoading(false);
    }
  }

  async function handleSaveSettings(e: React.FormEvent) {
    e.preventDefault();
    if (!repoUrl || !token) return;
    try {
      await invoke("save_credentials", { repoUrl, token });
      setShowSettings(false);
      alert("Settings saved securely!");
    } catch (e) {
      alert(`Failed to save: ${e}`);
    }
  }

  async function handleTriggerSync() {
    setSyncing(true);
    try {
      const res = await invoke<string>("trigger_sync");
      alert(res);
      await fetchBookmarks();
    } catch (e) {
      alert(`Sync Failed: ${e}`);
      setShowSettings(true); // Prompts to fix settings if credentials are off.
    } finally {
      setSyncing(false);
    }
  }

  return (
    <main className="min-h-screen bg-neutral-900 text-neutral-100 p-8 font-sans">
      <div className="max-w-4xl mx-auto space-y-8">
        <header className="flex justify-between items-center border-b border-neutral-800 pb-6">
          <h1 className="text-3xl font-bold tracking-tight text-white">
            Bookmarks<span className="text-blue-500">Sync</span>
          </h1>
          <div className="flex items-center gap-4">
            <div className="text-sm text-neutral-400">
              {bookmarks.length} items
            </div>
            <button
              onClick={handleTriggerSync}
              disabled={syncing}
              className="px-3 py-1.5 bg-green-600/20 text-green-400 hover:bg-green-600/30 rounded-md text-sm transition-colors border border-green-500/30 disabled:opacity-50"
            >
              {syncing ? "Syncing..." : "Sync ☁️"}
            </button>
            <button
              onClick={() => setShowSettings(!showSettings)}
              className="px-3 py-1.5 bg-neutral-800 hover:bg-neutral-700 rounded-md text-sm transition-colors border border-neutral-700"
            >
              ⚙️ Settings
            </button>
          </div>
        </header>

        {/* Settings Dialog */}
        {showSettings && (
          <section className="bg-neutral-800/80 p-6 rounded-xl border border-blue-500/30 shadow-2xl relative">
            <h2 className="text-lg font-medium text-white mb-4">Sync Configuration</h2>
            <form onSubmit={handleSaveSettings} className="space-y-4">
              <div>
                <label className="block text-sm text-neutral-400 mb-1">Git Repository SSH/HTTPS URL</label>
                <input
                  type="text"
                  className="w-full bg-neutral-900 border border-neutral-700 rounded-lg px-4 py-2 text-sm focus:outline-none focus:border-blue-500"
                  placeholder="e.g. https://github.com/user/my-bookmarks.git"
                  value={repoUrl}
                  onChange={e => setRepoUrl(e.target.value)}
                  required
                />
              </div>
              <div>
                <label className="block text-sm text-neutral-400 mb-1">GitHub Personal Access Token</label>
                <input
                  type="password"
                  className="w-full bg-neutral-900 border border-neutral-700 rounded-lg px-4 py-2 text-sm focus:outline-none focus:border-blue-500"
                  placeholder="ghp_xxxxxxxxxxxxxxxxxxxxx"
                  value={token}
                  onChange={e => setToken(e.target.value)}
                  required
                />
              </div>
              <div className="flex justify-end pt-2 gap-3">
                <button type="button" onClick={() => setShowSettings(false)} className="px-4 py-2 text-sm text-neutral-400 hover:text-white">Cancel</button>
                <button type="submit" className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm transition-colors shadow-lg">
                  Save Credentials
                </button>
              </div>
            </form>
          </section>
        )}

        {/* Search */}
        <section className="bg-neutral-800/50 p-6 rounded-xl border border-neutral-700/50 shadow-lg">
          <input
            type="text"
            className="w-full bg-neutral-900 border border-neutral-700 rounded-lg px-4 py-2.5 
                     text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500
                     transition-all shadow-inner"
            placeholder="🔍 Search bookmarks..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </section>

        {/* Add Actions */}
        <section className="bg-neutral-800/50 p-6 rounded-xl border border-neutral-700/50 shadow-lg">
          <form className="flex gap-4" onSubmit={handleAddBookmark}>
            <input
              type="url"
              className="flex-1 bg-neutral-900 border border-neutral-700 rounded-lg px-4 py-2.5 
                       text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500
                       transition-all shadow-inner"
              placeholder="https://example.com"
              value={newUrl}
              onChange={(e) => setNewUrl(e.currentTarget.value)}
              required
            />
            <button
              type="submit"
              disabled={loading}
              className="bg-blue-600 hover:bg-blue-500 text-white px-6 py-2.5 rounded-lg
                       font-medium text-sm transition-colors shadow-lg 
                       disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? "Adding..." : "Add Bookmark"}
            </button>
          </form>
        </section>

        {/* Bookmark Grid */}
        <section className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {bookmarks.length === 0 ? (
            <div className="col-span-full py-20 text-center text-neutral-500 border-2 border-dashed border-neutral-800 rounded-xl">
              No bookmarks yet. Add one above!
            </div>
          ) : (
            bookmarks.map((bm) => (
              <a
                href={bm.url}
                target="_blank"
                rel="noreferrer"
                key={bm.id}
                className="group flex flex-col p-5 bg-neutral-800 border border-neutral-700/80 
                         rounded-xl hover:border-blue-500/50 hover:bg-neutral-800/80
                         transition-all cursor-pointer shadow-sm relative overflow-hidden"
              >
                <div className="flex items-start gap-4 mb-3">
                  <div className="w-10 h-10 rounded bg-neutral-700 flex items-center justify-center shrink-0">
                    {/* Placeholder for real favicon later */}
                    <span className="text-lg font-bold text-neutral-400 uppercase">
                      {bm.host?.charAt(0) || "?"}
                    </span>
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="text-white font-medium truncate text-[15px] mb-1 group-hover:text-blue-400 transition-colors">
                      {bm.title || bm.url}
                    </h3>
                    <p className="text-sm text-neutral-400 truncate">
                      {bm.host}
                    </p>
                  </div>
                </div>
                <div className="mt-auto pt-4 border-t border-neutral-700/50 text-xs text-neutral-500 flex justify-between">
                  <span>{new Date(bm.created_at).toLocaleDateString()}</span>
                  <span className="truncate ml-4 max-w-[200px]" title={bm.url}>
                    {bm.url}
                  </span>
                </div>
              </a>
            ))
          )}
        </section>
      </div>
    </main>
  );
}

export default App;

import { memo } from "react";

export interface Folder { id: string; parent_id: string | null; name: string; }
export interface Tag { id: string; name: string; }

interface SidebarProps {
    folders: Folder[];
    tags: Tag[];
    selectedFolderId: string | null;
    selectedTagId: string | null;
    searchQuery: string;
    importing: boolean;
    onSelectAll: () => void;
    onSelectFolder: (id: string) => void;
    onSelectTag: (id: string) => void;
    onSearch: (q: string) => void;
    onImport: () => void;
    onNewFolder: () => void;
    onDeleteFolder: (e: React.MouseEvent, id: string) => void;
    onRenameFolder: (f: Folder) => void;
    onOpenSettings?: () => void;
}

export const Sidebar = memo(function Sidebar({
    folders,
    tags,
    selectedFolderId,
    selectedTagId,
    searchQuery,
    importing,
    onSelectAll,
    onSelectFolder,
    onSelectTag,
    onImport,
    onNewFolder,
    onDeleteFolder,
    onRenameFolder,
    onOpenSettings,
}: SidebarProps) {
    return (
        <aside className="w-full h-full border-r border-neutral-800 bg-neutral-950 flex flex-col xl:shadow-2xl">
            <div className="p-5 border-b border-neutral-800 flex items-center justify-between">
                <div>
                    <h1 className="logo-text text-3xl text-white tracking-widest font-black">拾页</h1>
                    <p className="text-[10px] text-neutral-500 mt-0.5 tracking-widest uppercase opacity-50">Local First</p>
                </div>
                {onOpenSettings && (
                    <button
                        aria-label="打开设置"
                        onClick={onOpenSettings}
                        className="w-8 h-8 flex items-center justify-center rounded-lg bg-neutral-800/60 text-neutral-500 hover:text-white hover:bg-neutral-700 transition-colors text-base"
                    >
                        ⚙️
                    </button>
                )}
            </div>

            <nav className="flex-1 overflow-y-auto p-4 space-y-1 scrollbar-hide">
                <button
                    onClick={onSelectAll}
                    className={`nav-item ${!selectedFolderId && !selectedTagId && !searchQuery ? "nav-item-active" : ""}`}
                >
                    🏠 全部书签
                </button>

                <div className="flex justify-between items-center pt-6 pb-2 px-4">
                    <div className="text-[10px] font-bold text-neutral-500 uppercase tracking-widest">文件夹</div>
                    <button onClick={onNewFolder} className="text-neutral-500 hover:text-white transition-colors">＋</button>
                </div>

                {folders.map(f => (
                    <div key={f.id} onClick={() => onSelectFolder(f.id)}
                        className={`group nav-item flex items-center justify-between cursor-pointer ${selectedFolderId === f.id ? "nav-item-active font-medium" : ""}`}>
                        <span className="truncate flex-1 flex items-center gap-2">
                            <span className="opacity-50">📁</span> {f.name}
                        </span>
                        <div className="opacity-100 transition-all duration-200 flex items-center gap-1">
                            <button
                                type="button"
                                aria-label="重命名文件夹"
                                onClick={(e) => { e.stopPropagation(); onRenameFolder(f); }}
                                title="重命名文件夹"
                                className="p-1.5 rounded-lg hover:bg-neutral-700/50 hover:text-white text-neutral-500"
                            >
                                ✏️
                            </button>
                            <button
                                type="button"
                                aria-label="删除文件夹"
                                onClick={(e) => onDeleteFolder(e, f.id)}
                                title="删除文件夹"
                                className="p-1.5 rounded-lg hover:bg-red-500/20 hover:text-red-500 transition-all duration-200 text-neutral-500"
                            >
                                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 6h18"></path><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path></svg>
                            </button>
                        </div>
                    </div>
                ))}

                <div className="pt-6 pb-2 px-4 text-[10px] font-bold text-neutral-500 uppercase tracking-widest">标签</div>
                <div className="flex flex-wrap gap-1 px-3">
                    {tags.map(t => (
                        <button key={t.id} onClick={() => onSelectTag(t.id)}
                            className={`tag-item ${selectedTagId === t.id ? "tag-item-active" : "tag-item-inactive"}`}>
                            # {t.name}
                        </button>
                    ))}
                </div>
            </nav>

            <div className="p-6 border-t border-neutral-800 space-y-3 bg-neutral-950/50">
                <button onClick={onImport} disabled={importing} className="btn-base btn-neutral w-full py-3">
                    {importing ? "同步中..." : "📥 同步本地浏览器"}
                </button>
            </div>
        </aside>
    );
});

import { memo } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

export interface Bookmark {
    id: string;
    url: string;
    title?: string;
    description?: string;
    favicon_url?: string;
    host?: string;
    created_at: string;
    tags?: string[];
}

interface BookmarkItemProps {
    bm: Bookmark;
    isSelected?: boolean;
    onSelect: (bm: Bookmark) => void;
    onAddTag: (id: string) => void;
    onEdit: (bm: Bookmark) => void;
    onDelete: (id: string) => void;
}

export const BookmarkItem = memo(function BookmarkItem({ bm, isSelected, onSelect, onAddTag, onEdit, onDelete }: BookmarkItemProps) {
    async function openBookmarkInDefaultBrowser(url: string) {
        try {
            await openUrl(url);
        } catch (e) {
            console.error(e);
            alert(`打开链接失败：${String(e)}`);
        }
    }

    return (
        <div
            onClick={() => onSelect(bm)}
            className={`group flex flex-col p-4 rounded-2xl cursor-pointer transition-all border ${isSelected ? 'bg-neutral-800/80 border-neutral-600 shadow-md transform scale-[1.01]' : 'bg-neutral-900/40 border-neutral-800 hover:bg-neutral-800/60 hover:border-neutral-700'}`}
        >
            <div className="flex gap-3 items-start relative">
                <div className="w-10 h-10 rounded-xl bg-neutral-950 border border-neutral-800 flex items-center justify-center shrink-0 overflow-hidden shadow-inner mt-1">
                    {bm.favicon_url ? (
                        <img src={bm.favicon_url} className="w-5 h-5 object-contain" alt="" />
                    ) : (
                        <span className="text-sm font-bold text-neutral-800 uppercase">{bm.host?.charAt(0) || "?"}</span>
                    )}
                </div>

                <div className="flex-1 min-w-0 pb-1">
                    <h3
                        className={`font-semibold truncate mb-1 transition-colors text-sm ${isSelected ? 'text-white' : 'text-neutral-200'}`}
                        onDoubleClick={(e) => { e.stopPropagation(); openBookmarkInDefaultBrowser(bm.url); }}
                    >
                        {bm.title || bm.url}
                    </h3>
                    <p className="text-[11px] text-neutral-500 truncate font-medium mb-2">{bm.host}</p>
                    <div className="flex flex-wrap gap-1.5">
                        {bm.tags?.map(t => <span key={t} className="px-2 py-0.5 rounded-md text-[10px] font-medium bg-neutral-800 text-neutral-400 border border-neutral-700 shadow-sm">#{t}</span>)}
                    </div>
                </div>

                <div className="flex gap-1.5 absolute top-0 right-0 opacity-0 group-hover:opacity-100 transition-all bg-linear-to-l from-neutral-900 via-neutral-900/80 to-transparent pl-6">
                    <button aria-label={`新增标签-${bm.title || bm.url}`} onClick={(e) => { e.stopPropagation(); onAddTag(bm.id); }} className="btn-icon p-1.5 rounded-lg bg-neutral-800 text-neutral-400 hover:text-white hover:bg-neutral-700 transition">＋</button>
                    <button aria-label={`编辑书签-${bm.title || bm.url}`} onClick={(e) => { e.stopPropagation(); onEdit(bm); }} className="btn-icon p-1.5 rounded-lg bg-neutral-800 text-neutral-400 hover:text-white hover:bg-neutral-700 transition">✏️</button>
                    <button aria-label="删除书签" onClick={(e) => { e.stopPropagation(); onDelete(bm.id); }} className="btn-icon btn-danger p-1.5 rounded-lg bg-neutral-800 text-neutral-400 hover:text-red-400 hover:bg-red-950/50 transition">🗑️</button>
                </div>
            </div>
        </div>
    );
});

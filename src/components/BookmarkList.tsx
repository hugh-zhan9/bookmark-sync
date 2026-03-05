import { memo } from "react";
import { Bookmark, BookmarkItem } from "./BookmarkItem";

interface BookmarkListProps {
    bookmarks: Bookmark[];
    newUrl: string;
    loading: boolean;
    searchQuery: string;
    selectedBookmarkId: string | null;
    onNewUrlChange: (val: string) => void;
    onSearchQueryChange: (val: string) => void;
    onAddBookmark: (e: React.FormEvent) => void;
    onSelectBookmark: (bm: Bookmark) => void;
    onAddTag: (id: string) => void;
    onEdit: (bm: Bookmark) => void;
    onDelete: (id: string) => void;
}

export const BookmarkList = memo(function BookmarkList({
    bookmarks,
    newUrl,
    loading,
    searchQuery,
    selectedBookmarkId,
    onNewUrlChange,
    onSearchQueryChange,
    onAddBookmark,
    onSelectBookmark,
    onAddTag,
    onEdit,
    onDelete
}: BookmarkListProps) {
    return (
        <div className="flex-1 flex flex-col h-full bg-neutral-900 overflow-hidden relative border-r border-neutral-800">
            <header className="p-5 border-b border-neutral-800/80 bg-neutral-900/60 backdrop-blur-xl z-20 shrink-0">
                <div className="flex gap-4">
                    <div className="w-1/3 relative text-white">
                        <span className="absolute left-4 top-1/2 -translate-y-1/2 text-neutral-500">🔍</span>
                        <input
                            type="text"
                            placeholder="搜索标题、域名或标签..."
                            className="input-field input-field-leading w-full bg-neutral-950/50 border-neutral-800 text-sm py-2.5 h-auto rounded-xl"
                            value={searchQuery}
                            onChange={e => onSearchQueryChange(e.target.value)}
                        />
                    </div>
                    <form className="flex-1 flex gap-3" onSubmit={onAddBookmark}>
                        <input
                            type="url"
                            placeholder="粘贴 URL 快速添加书签..."
                            className="input-field flex-1 bg-neutral-950/50 border-neutral-800 text-sm py-2.5 h-auto rounded-xl"
                            value={newUrl}
                            onChange={e => onNewUrlChange(e.target.value)}
                            required
                        />
                        <button disabled={loading} className="btn-base btn-accent px-5 py-2.5 text-sm h-auto rounded-xl whitespace-nowrap">
                            {loading ? "..." : "添加"}
                        </button>
                    </form>
                </div>
            </header>

            <div className="flex-1 overflow-y-auto p-6 scrollbar-hide z-10">
                <div className="grid grid-cols-1 gap-4">
                    {bookmarks.map(bm => (
                        <BookmarkItem
                            key={bm.id}
                            bm={bm}
                            isSelected={selectedBookmarkId === bm.id}
                            onSelect={onSelectBookmark}
                            onAddTag={onAddTag}
                            onEdit={onEdit}
                            onDelete={onDelete}
                        />
                    ))}
                </div>
            </div>
        </div>
    );
});

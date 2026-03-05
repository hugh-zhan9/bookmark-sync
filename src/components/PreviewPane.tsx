import { useState, useEffect } from "react";
import { Bookmark } from "./BookmarkItem";
import { openUrl } from "@tauri-apps/plugin-opener";

interface PreviewPaneProps {
    bookmark: Bookmark | null;
    onClose: () => void;
}

export function PreviewPane({ bookmark, onClose }: PreviewPaneProps) {
    const [iframeError, setIframeError] = useState(false);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        // 切换书签时，重置状态
        setIframeError(false);
        if (bookmark) {
            setLoading(true);
        }
    }, [bookmark?.id]);

    if (!bookmark) {
        return (
            <div className="flex-1 h-full bg-neutral-950 flex flex-col items-center justify-center text-neutral-600">
                <div className="w-24 h-24 mb-6 opacity-20 bg-neutral-800 rounded-3xl flex items-center justify-center">
                    <span className="text-4xl text-neutral-400">👀</span>
                </div>
                <p className="text-sm font-medium tracking-widest">在左侧选择书签以预览内容</p>
            </div>
        );
    }

    async function handleOpenExternal() {
        try {
            await openUrl(bookmark!.url);
        } catch (e) {
            console.error(e);
            alert(`打开链接失败：${String(e)}`);
        }
    }

    return (
        <div className="flex-1 h-full flex flex-col bg-white overflow-hidden relative shadow-2xl">
            {/* 顶部控制台 */}
            <header className="h-14 shrink-0 bg-neutral-100 border-b border-neutral-200 flex items-center justify-between px-4 z-20">
                <div className="flex items-center gap-3 overflow-hidden">
                    <button onClick={onClose} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-neutral-200 text-neutral-500 transition-colors">
                        <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M18 6 6 18" /><path d="m6 6 12 12" /></svg>
                    </button>
                    <div className="w-6 h-6 rounded bg-neutral-200 border border-neutral-300 flex items-center justify-center shrink-0 overflow-hidden">
                        {bookmark.favicon_url ? (
                            <img src={bookmark.favicon_url} className="w-4 h-4 object-contain" alt="" />
                        ) : (
                            <span className="text-[10px] font-bold text-neutral-600 uppercase">{bookmark.host?.charAt(0) || "?"}</span>
                        )}
                    </div>
                    <span className="text-sm font-semibold text-neutral-800 truncate" title={bookmark.title || bookmark.url}>
                        {bookmark.title || bookmark.host}
                    </span>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                    <button
                        onClick={handleOpenExternal}
                        className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-neutral-600 bg-white border border-neutral-200 rounded-md hover:bg-neutral-50 hover:text-neutral-900 transition-colors shadow-sm"
                    >
                        <span>在浏览器打开</span>
                        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path><polyline points="15 3 21 3 21 9"></polyline><line x1="10" y1="14" x2="21" y2="3"></line></svg>
                    </button>
                </div>
            </header>

            {/* 预览区域 */}
            <div className="flex-1 relative bg-neutral-50">
                {loading && !iframeError && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center bg-neutral-50 z-10">
                        <div className="w-8 h-8 border-2 border-neutral-300 border-t-blue-500 rounded-full animate-spin mb-4"></div>
                        <p className="text-xs text-neutral-500 font-medium tracking-widest uppercase">Loading...</p>
                    </div>
                )}

                {iframeError ? (
                    // 降级视图
                    <div className="absolute inset-0 flex flex-col items-center justify-center p-8 bg-neutral-950 text-neutral-200">
                        <div className="w-full max-w-md bg-neutral-900 rounded-3xl p-8 border border-neutral-800 shadow-2xl flex flex-col items-center text-center">
                            <div className="w-20 h-20 rounded-2xl bg-neutral-800 border-2 border-neutral-700 flex items-center justify-center mb-6 shadow-inner">
                                {bookmark.favicon_url ? (
                                    <img src={bookmark.favicon_url} className="w-10 h-10 object-contain" alt="" />
                                ) : (
                                    <span className="text-2xl font-bold text-neutral-500 uppercase">{bookmark.host?.charAt(0) || "?"}</span>
                                )}
                            </div>
                            <h2 className="text-xl font-bold text-white mb-2 line-clamp-2">{bookmark.title || bookmark.url}</h2>
                            <p className="text-sm text-neutral-500 mb-8">{bookmark.host}</p>

                            <div className="bg-orange-500/10 text-orange-400 border border-orange-500/20 rounded-xl p-4 mb-8 text-xs text-left w-full leading-relaxed">
                                <p className="font-semibold mb-1 flex items-center gap-1.5"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" /><path d="M12 9v4" /><path d="M12 17h.01" /></svg> 无法在应用内预览</p>
                                目标网站的安全策略（X-Frame-Options）阻止了该页面的嵌入加载。请在外部浏览器中继续阅读。
                            </div>

                            <button
                                onClick={handleOpenExternal}
                                className="w-full py-4 bg-white text-black font-semibold rounded-2xl hover:bg-neutral-200 transition-colors shadow-lg active:scale-95 duration-200"
                            >
                                使用外部浏览器打开
                            </button>
                        </div>
                    </div>
                ) : (
                    <iframe
                        key={bookmark.id} // 强制重建 iframe
                        className="w-full h-full border-0 bg-white"
                        src={bookmark.url}
                        sandbox="allow-same-origin allow-scripts allow-popups allow-forms"
                        onLoad={() => setLoading(false)}
                        onError={() => {
                            setIframeError(true);
                            setLoading(false);
                        }}
                    />
                )}
            </div>
        </div>
    );
}

import { useRef, useState, useCallback, useEffect } from "react";

interface ResizableLayoutProps {
    sidebar: React.ReactNode;
    list: React.ReactNode;
    preview: React.ReactNode;
}

/**
 * 纯原生三栏可拖拽布局（性能优化版）
 * 拖拽时直接操作 DOM 宽度，不触发 React 重渲染；
 * 只在 mouseup 后更新 React state（确保子组件 key 稳定）
 * onMouseMove / onMouseUp 均为空依赖，全局监听器只注册一次。
 */
export function ResizableLayout({ sidebar, list, preview }: ResizableLayoutProps) {
    const [sizes, setSizes] = useState({ sidebar: 20, list: 30 });

    const containerRef = useRef<HTMLDivElement>(null);
    const sidebarRef = useRef<HTMLDivElement>(null);
    const listRef = useRef<HTMLDivElement>(null);
    const previewRef = useRef<HTMLDivElement>(null);
    const draggingRef = useRef<"left" | "right" | null>(null);
    const startXRef = useRef(0);
    const startSizesRef = useRef({ sidebar: 20, list: 30 });

    // 用 ref 缓存最新 sizes，让 onMouseUp 无需依赖 sizes state
    const sizesRef = useRef(sizes);
    useEffect(() => { sizesRef.current = sizes; }, [sizes]);

    // 直接操作 DOM 宽度，不触发 React re-render
    const applyWidths = useCallback((sidebarPct: number, listPct: number) => {
        const previewPct = 100 - sidebarPct - listPct;
        if (sidebarRef.current) sidebarRef.current.style.width = `${sidebarPct}%`;
        if (listRef.current) listRef.current.style.width = `${listPct}%`;
        if (previewRef.current) previewRef.current.style.width = `${previewPct}%`;
    }, []);

    const onMouseMove = useCallback((e: MouseEvent) => {
        const type = draggingRef.current;
        if (!type) return;
        const container = containerRef.current;
        if (!container) return;

        const totalW = container.offsetWidth;
        const dx = e.clientX - startXRef.current;
        const delta = (dx / totalW) * 100;
        const { sidebar: s0, list: l0 } = startSizesRef.current;

        if (type === "left") {
            const newSidebar = Math.min(35, Math.max(12, s0 + delta));
            const newList = Math.min(50, Math.max(20, l0 - delta));
            applyWidths(newSidebar, newList);
        } else {
            const newList = Math.min(50, Math.max(20, l0 + delta));
            applyWidths(s0, newList);
        }
    }, [applyWidths]);

    // 空依赖：从 DOM 读取最终宽度，不依赖 sizes state，监听器只注册一次
    const onMouseUp = useCallback(() => {
        if (!draggingRef.current) return;
        draggingRef.current = null;
        document.body.style.userSelect = "";
        document.body.style.cursor = "";

        // 从 DOM 直接读取最终宽度，同步到 React state
        if (sidebarRef.current && listRef.current) {
            const newSidebar = parseFloat(sidebarRef.current.style.width) || sizesRef.current.sidebar;
            const newList = parseFloat(listRef.current.style.width) || sizesRef.current.list;
            setSizes({ sidebar: newSidebar, list: newList });
        }
    }, []); // 空依赖 —— 全局监听器只在挂载/卸载时注册一次

    useEffect(() => {
        window.addEventListener("mousemove", onMouseMove);
        window.addEventListener("mouseup", onMouseUp);
        return () => {
            window.removeEventListener("mousemove", onMouseMove);
            window.removeEventListener("mouseup", onMouseUp);
        };
    }, [onMouseMove, onMouseUp]);

    const startDrag = useCallback((type: "left" | "right", e: React.MouseEvent) => {
        e.preventDefault();
        draggingRef.current = type;
        startXRef.current = e.clientX;
        startSizesRef.current = { sidebar: sizes.sidebar, list: sizes.list };
        document.body.style.userSelect = "none";
        document.body.style.cursor = "col-resize";
    }, [sizes]);

    const previewPct = 100 - sizes.sidebar - sizes.list;

    const dividerStyle: React.CSSProperties = {
        width: 4,
        flexShrink: 0,
        cursor: "col-resize",
        background: "rgba(82, 82, 91, 0.5)",
        zIndex: 20,
        transition: "background 0.15s",
    };

    return (
        <div
            ref={containerRef}
            style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "row", overflow: "hidden" }}
        >
            {/* 左栏 Sidebar */}
            <div ref={sidebarRef} style={{ width: `${sizes.sidebar}%`, minWidth: 0, overflow: "hidden", flexShrink: 0 }}>
                {sidebar}
            </div>

            {/* 分隔线 1 */}
            <div
                style={dividerStyle}
                onMouseDown={e => startDrag("left", e)}
                onMouseEnter={e => (e.currentTarget.style.background = "rgba(96,165,250,0.7)")}
                onMouseLeave={e => (e.currentTarget.style.background = "rgba(82,82,91,0.5)")}
            />

            {/* 中栏 BookmarkList */}
            <div ref={listRef} style={{ width: `${sizes.list}%`, minWidth: 0, overflow: "hidden", flexShrink: 0 }}>
                {list}
            </div>

            {/* 分隔线 2 */}
            <div
                style={dividerStyle}
                onMouseDown={e => startDrag("right", e)}
                onMouseEnter={e => (e.currentTarget.style.background = "rgba(96,165,250,0.7)")}
                onMouseLeave={e => (e.currentTarget.style.background = "rgba(82,82,91,0.5)")}
            />

            {/* 右栏 Preview */}
            <div ref={previewRef} style={{ width: `${previewPct}%`, minWidth: 0, overflow: "hidden", flexGrow: 1 }}>
                {preview}
            </div>
        </div>
    );
}

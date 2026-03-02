// The literal name recognized by your Native Messaging Host (Tauri application)
const NATIVE_HOST_NAME = "com.bookmark.sync.client";

let nativePort = null;

function connectToNativeHost() {
    nativePort = chrome.runtime.connectNative(NATIVE_HOST_NAME);

    nativePort.onMessage.addListener((msg) => {
        console.log("Received a message from native Rust Host:", msg);
    });

    nativePort.onDisconnect.addListener(() => {
        console.warn("Disconnected from native host. Error:", chrome.runtime.lastError);
        nativePort = null;

        // Auto-reconnect after 5s
        setTimeout(connectToNativeHost, 5000);
    });
}

// Ensure connection stays alive
connectToNativeHost();


// ---- Bookmark Change Listeners ----

const serializeBookmarkNode = (node) => {
    return {
        id: node.id,
        url: node.url || "",
        title: node.title || "",
        parentId: node.parentId,
        dateAdded: node.dateAdded
    };
}

chrome.bookmarks.onCreated.addListener((id, bookmark) => {
    console.log("Bookmark created/added:", bookmark);
    if (!nativePort) return;

    nativePort.postMessage({
        type: "BookmarkAdded",
        payload: serializeBookmarkNode(bookmark)
    });
});

chrome.bookmarks.onRemoved.addListener((id, removeInfo) => {
    console.log("Bookmark removed:", id, removeInfo);
    if (!nativePort) return;

    nativePort.postMessage({
        type: "BookmarkDeleted",
        payload: {
            id: removeInfo.node.id,
            url: removeInfo.node.url,
            title: removeInfo.node.title
        }
    });
});

chrome.bookmarks.onChanged.addListener((id, changeInfo) => {
    console.log("Bookmark changed:", id, changeInfo);
    if (!nativePort) return;

    nativePort.postMessage({
        type: "BookmarkUpdated",
        payload: {
            id,
            url: changeInfo.url,
            title: changeInfo.title
        }
    });
});

const NATIVE_HOST_NAME = "com.bookmark.sync.client";

let nativePort = null;

function serializeBookmarkNode(node) {
  return {
    id: node.id,
    url: node.url || "",
    title: node.title || "",
    parentId: node.parentId,
    dateAdded: node.dateAdded,
  };
}

function flattenBookmarkTree(tree) {
  const result = [];

  function walk(nodes) {
    if (!Array.isArray(nodes)) return;
    for (const node of nodes) {
      if (node && node.url) {
        result.push(serializeBookmarkNode(node));
      }
      if (node && Array.isArray(node.children)) {
        walk(node.children);
      }
    }
  }

  walk(tree);
  return result;
}

function postToHost(message) {
  if (!nativePort) return false;
  try {
    nativePort.postMessage(message);
    return true;
  } catch (e) {
    console.warn("Failed to post native message:", e);
    return false;
  }
}

function triggerFullSync(reason) {
  chrome.bookmarks.getTree((tree) => {
    if (chrome.runtime.lastError) {
      console.warn("getTree failed:", chrome.runtime.lastError);
      return;
    }

    const bookmarks = flattenBookmarkTree(tree || []);
    postToHost({
      type: "FullSync",
      payload: {
        reason,
        bookmarks,
        syncedAt: new Date().toISOString(),
      },
    });
  });
}

function captureActiveTabToApp() {
  chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
    if (chrome.runtime.lastError) {
      console.warn("tabs.query failed:", chrome.runtime.lastError);
      return;
    }

    const tab = tabs && tabs[0];
    if (!tab || !tab.url) return;

    postToHost({
      type: "PageCaptured",
      payload: {
        id: tab.id ? String(tab.id) : "",
        url: tab.url,
        title: tab.title || tab.url,
        source: "action_click",
        capturedAt: new Date().toISOString(),
      },
    });
  });
}

function connectToNativeHost() {
  nativePort = chrome.runtime.connectNative(NATIVE_HOST_NAME);

  nativePort.onMessage.addListener((msg) => {
    console.log("Received a message from native host:", msg);
  });

  nativePort.onDisconnect.addListener(() => {
    console.warn("Disconnected from native host. Error:", chrome.runtime.lastError);
    nativePort = null;
    setTimeout(connectToNativeHost, 5000);
  });

  triggerFullSync("native_connected");
}

connectToNativeHost();

chrome.runtime.onInstalled.addListener((details) => {
  triggerFullSync(`onInstalled:${details.reason || "unknown"}`);
});

chrome.runtime.onStartup.addListener(() => {
  triggerFullSync("onStartup");
});

chrome.action.onClicked.addListener(() => {
  captureActiveTabToApp();
});

chrome.bookmarks.onCreated.addListener((_id, bookmark) => {
  postToHost({
    type: "BookmarkAdded",
    payload: serializeBookmarkNode(bookmark),
  });
});

chrome.bookmarks.onRemoved.addListener((_id, removeInfo) => {
  postToHost({
    type: "BookmarkDeleted",
    payload: {
      id: removeInfo.node.id,
      url: removeInfo.node.url,
      title: removeInfo.node.title,
    },
  });
});

chrome.bookmarks.onChanged.addListener((id, changeInfo) => {
  postToHost({
    type: "BookmarkUpdated",
    payload: {
      id,
      url: changeInfo.url,
      title: changeInfo.title,
    },
  });
});

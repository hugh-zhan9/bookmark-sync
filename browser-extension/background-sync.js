function flattenBookmarkTree(tree) {
  const out = [];
  const walk = (nodes) => {
    if (!Array.isArray(nodes)) return;
    for (const node of nodes) {
      if (node && node.url) {
        out.push({
          id: node.id,
          url: node.url,
          title: node.title || '',
          parentId: node.parentId,
          dateAdded: node.dateAdded,
        });
      }
      if (node && Array.isArray(node.children)) {
        walk(node.children);
      }
    }
  };
  walk(tree);
  return out;
}

function buildPageCapturedMessage(tab) {
  return {
    type: 'PageCaptured',
    payload: {
      id: String(tab.id || ''),
      url: tab.url || '',
      title: tab.title || '',
      source: 'action_click',
      capturedAt: new Date().toISOString(),
    },
  };
}

if (typeof module !== 'undefined') {
  module.exports = {
    flattenBookmarkTree,
    buildPageCapturedMessage,
  };
}

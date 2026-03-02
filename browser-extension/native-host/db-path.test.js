const test = require('node:test');
const assert = require('node:assert/strict');

const { getDbPath } = require('./db-path');

test('macOS 优先使用 identifier 对应目录', () => {
  const homeDir = '/Users/demo';
  const idPath = '/Users/demo/Library/Application Support/com.zhangyukun.bookmark-sync-app/bookmarks.db';
  const legacyPath = '/Users/demo/Library/Application Support/拾页/bookmarks.db';

  const picked = getDbPath({
    platform: 'darwin',
    homeDir,
    appIdentifier: 'com.zhangyukun.bookmark-sync-app',
    legacyAppName: '拾页',
    existsSync: (p) => p === idPath || p === legacyPath,
  });

  assert.equal(picked, idPath);
});

test('macOS 在 identifier 不存在时回退 legacy 目录', () => {
  const homeDir = '/Users/demo';
  const legacyPath = '/Users/demo/Library/Application Support/拾页/bookmarks.db';

  const picked = getDbPath({
    platform: 'darwin',
    homeDir,
    appIdentifier: 'com.zhangyukun.bookmark-sync-app',
    legacyAppName: '拾页',
    existsSync: (p) => p === legacyPath,
  });

  assert.equal(picked, legacyPath);
});

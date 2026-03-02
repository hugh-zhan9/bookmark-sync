const path = require('path');
const os = require('os');
const fs = require('fs');

function resolveDbPathCandidates(options = {}) {
  const platform = options.platform || process.platform;
  const homeDir = options.homeDir || os.homedir();
  const appIdentifier = options.appIdentifier || process.env.SHIIYE_APP_IDENTIFIER || 'com.zhangyukun.bookmark-sync-app';
  const legacyAppName = options.legacyAppName || process.env.SHIIYE_APP_NAME || '拾页';

  if (platform === 'darwin') {
    return [
      path.join(homeDir, 'Library', 'Application Support', appIdentifier, 'bookmarks.db'),
      path.join(homeDir, 'Library', 'Application Support', legacyAppName, 'bookmarks.db'),
    ];
  }

  if (platform === 'win32') {
    const appData = options.appData || process.env.APPDATA || '';
    return [
      path.join(appData, appIdentifier, 'bookmarks.db'),
      path.join(appData, legacyAppName, 'bookmarks.db'),
    ];
  }

  return [
    path.join(homeDir, '.local', 'share', appIdentifier, 'bookmarks.db'),
    path.join(homeDir, '.local', 'share', legacyAppName, 'bookmarks.db'),
  ];
}

function getDbPath(options = {}) {
  const existsSync = options.existsSync || fs.existsSync;
  const candidates = resolveDbPathCandidates(options);

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }

  return candidates[0];
}

module.exports = {
  resolveDbPathCandidates,
  getDbPath,
};

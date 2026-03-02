const test = require('node:test');
const assert = require('node:assert/strict');
const Database = require('better-sqlite3');

const { upsertBookmarksBatch } = require('./db-writer');

test('全量同步应批量写入且对同 URL 幂等去重', () => {
  const db = new Database(':memory:');
  db.exec(`
    CREATE TABLE bookmarks (
      id TEXT PRIMARY KEY,
      url TEXT UNIQUE NOT NULL,
      canonical_url TEXT UNIQUE NOT NULL,
      title TEXT,
      host TEXT,
      created_at TEXT,
      updated_at TEXT,
      is_deleted BOOLEAN DEFAULT 0
    );
  `);

  db.prepare(`INSERT INTO bookmarks (id,url,canonical_url,title,host,created_at,updated_at,is_deleted)
              VALUES ('old-id','https://example.com','https://example.com','old','example.com','2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z',1)`).run();

  const now = '2026-03-02T08:00:00.000Z';
  upsertBookmarksBatch(db, [
    { id: 'a1', url: 'https://example.com', title: 'new title' },
    { id: 'a2', url: 'https://v2ex.com', title: 'v2ex' },
  ], now);

  const row1 = db.prepare('SELECT id,title,is_deleted,updated_at FROM bookmarks WHERE url=?').get('https://example.com');
  assert.equal(row1.id, 'old-id');
  assert.equal(row1.title, 'new title');
  assert.equal(row1.is_deleted, 0);
  assert.equal(row1.updated_at, now);

  const row2 = db.prepare('SELECT id,title,is_deleted FROM bookmarks WHERE url=?').get('https://v2ex.com');
  assert.equal(row2.id, 'a2');
  assert.equal(row2.title, 'v2ex');
  assert.equal(row2.is_deleted, 0);
});

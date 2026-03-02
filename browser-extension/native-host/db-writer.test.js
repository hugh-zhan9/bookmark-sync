const test = require('node:test');
const assert = require('node:assert/strict');
const Database = require('better-sqlite3');

const { upsertBookmarkAdded } = require('./db-writer');

test('删除后再次添加相同 URL 应恢复为未删除并更新时间', () => {
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

  const now = '2026-03-02T06:00:00.000Z';
  upsertBookmarkAdded(db, { id: 'new-id', url: 'https://example.com', title: 'new title', now });

  const row = db.prepare('SELECT id,url,title,is_deleted,updated_at FROM bookmarks WHERE url=?').get('https://example.com');
  assert.equal(row.id, 'old-id');
  assert.equal(row.title, 'new title');
  assert.equal(row.is_deleted, 0);
  assert.equal(row.updated_at, now);
});

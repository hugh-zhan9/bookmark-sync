function upsertBookmarkAdded(db, { id, url, title, now }) {
  if (!url || url === '') return;

  db.prepare(`
    INSERT INTO bookmarks (id, url, canonical_url, title, host, created_at, updated_at, is_deleted)
    VALUES (?, ?, ?, ?, ?, ?, ?, 0)
    ON CONFLICT(url) DO UPDATE SET
      canonical_url = excluded.canonical_url,
      title = excluded.title,
      host = excluded.host,
      updated_at = excluded.updated_at,
      is_deleted = 0
  `).run(
    id || require('crypto').randomUUID(),
    url,
    url,
    title || url,
    new URL(url).hostname,
    now,
    now
  );
}

module.exports = {
  upsertBookmarkAdded,
};

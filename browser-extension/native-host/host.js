#!/usr/bin/env node
/**
 * 拾页 Native Messaging Host
 *
 * 被 Chrome 直接 fork，通过 stdio 与浏览器扩展通信。
 * 接到书签事件后，直接写入 App 共享的 SQLite 数据库。
 *
 * 数据库路径优先匹配 Tauri identifier（兼容历史目录回退）：
 *   macOS: ~/Library/Application Support/com.zhangyukun.bookmark-sync-app/bookmarks.db
 */

const path = require('path');
const os = require('os');
const fs = require('fs');
const { getDbPath } = require('./db-path');
const { upsertBookmarkAdded } = require('./db-writer');

// ---- 日志（写文件，避免污染 stdout）----
const LOG_FILE = path.join(os.tmpdir(), 'shiiye-native-host.log');
function log(msg) {
    fs.appendFileSync(LOG_FILE, new Date().toISOString() + ' ' + msg + '\n');
}

// ---- 动态加载 better-sqlite3 ----
let db;
try {
    const Database = require('better-sqlite3');
    const dbPath = getDbPath();
    // 等 Tauri App 初始化好数据库文件，最多等待 5 秒
    let attempts = 0;
    while (!fs.existsSync(dbPath) && attempts < 10) {
        const start = Date.now();
        while (Date.now() - start < 500) { } // 同步等 500ms
        attempts++;
    }
    if (fs.existsSync(dbPath)) {
        db = new Database(dbPath);
    } else {
        log('DB not found at ' + dbPath + ', will buffer events to log file');
    }
} catch (e) {
    log('SQLite not available: ' + e.message);
}

// ---- 回退：事件写入本地 JSON log，等 App 扫描 ----
const QUEUE_FILE = path.join(path.dirname(getDbPath()), 'pending_events.jsonl');

function saveToQueue(event) {
    try {
        const dir = path.dirname(QUEUE_FILE);
        if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
        fs.appendFileSync(QUEUE_FILE, JSON.stringify(event) + '\n');
        log('Saved event to queue: ' + JSON.stringify(event));
    } catch (e) {
        log('Failed to save queue: ' + e.message);
    }
}

// ---- 写入 SQLite ----
function handleBookmarkEvent(msg) {
    const { type, payload } = msg;
    log('Received: ' + JSON.stringify(msg));

    if (!db) {
        saveToQueue(msg);
        return;
    }

    try {
        if (type === 'BookmarkAdded') {
            const { id, url, title } = payload;
            const now = new Date().toISOString();
            upsertBookmarkAdded(db, { id, url, title, now });
            log('Inserted bookmark: ' + url);
        } else if (type === 'BookmarkDeleted') {
            const { id, url } = payload;
            if (id) {
                db.prepare('UPDATE bookmarks SET is_deleted = 1 WHERE id = ?').run(id);
            } else if (url) {
                db.prepare('UPDATE bookmarks SET is_deleted = 1 WHERE url = ?').run(url);
            }
            log('Deleted bookmark: ' + (id || url));
        } else if (type === 'BookmarkUpdated') {
            const { id, title, url } = payload;
            if (title) db.prepare('UPDATE bookmarks SET title = ? WHERE id = ?').run(title, id);
            if (url) db.prepare('UPDATE bookmarks SET url = ? WHERE id = ?').run(url, id);
            log('Updated bookmark: ' + id);
        }
    } catch (e) {
        log('DB error: ' + e.message);
        saveToQueue(msg);
    }
}

// ---- Chrome Native Messaging 协议：4 字节长度前缀 + JSON ----
function readMessage(callback) {
    const lengthBuffer = Buffer.alloc(4);
    let bytesRead = 0;

    function readLoop() {
        const chunk = process.stdin.read(4 - bytesRead);
        if (chunk) {
            chunk.copy(lengthBuffer, bytesRead);
            bytesRead += chunk.length;
        }
        if (bytesRead < 4) {
            process.stdin.once('readable', readLoop);
            return;
        }
        const msgLen = lengthBuffer.readUInt32LE(0);
        const msgBuffer = process.stdin.read(msgLen);
        if (!msgBuffer || msgBuffer.length < msgLen) {
            process.stdin.once('readable', () => {
                const rest = process.stdin.read(msgLen);
                if (rest) {
                    try {
                        callback(JSON.parse(rest.toString('utf8')));
                    } catch (e) { log('JSON parse error: ' + e); }
                }
                bytesRead = 0;
                readLoop();
            });
            return;
        }
        try {
            callback(JSON.parse(msgBuffer.toString('utf8')));
        } catch (e) {
            log('JSON parse error: ' + e);
        }
        bytesRead = 0;
        readLoop();
    }

    process.stdin.once('readable', readLoop);
}

// ---- 向 Chrome 发送回包（简单 ACK）----
function sendMessage(msg) {
    const msgStr = JSON.stringify(msg);
    const buf = Buffer.alloc(4 + msgStr.length);
    buf.writeUInt32LE(msgStr.length, 0);
    buf.write(msgStr, 4);
    process.stdout.write(buf);
}

// ---- 启动 ----
function startHost() {
    log('拾页 Native Messaging Host started. DB: ' + getDbPath());
    process.stdin.resume();
    process.stdin.on('end', () => {
        log('stdin closed, exiting.');
        process.exit(0);
    });

    readMessage(function onMessage(msg) {
        handleBookmarkEvent(msg);
        sendMessage({ status: 'ok' });
    });
}

if (require.main === module) {
    startHost();
}

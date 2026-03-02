#!/usr/bin/env node

/**
 * A utility script to read the local Chrome Bookmarks file on macOS
 * and output a flattened JSON array simulating the Event Log format.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const CHROME_BOOKMARKS_PATH = path.join(
    process.env.HOME,
    'Library/Application Support/Google/Chrome/Default/Bookmarks'
);

function traverseBookmarks(node, results = []) {
    if (node.type === 'url') {
        let host = '';
        try {
            host = new URL(node.url).hostname;
        } catch (e) { }

        results.push({
            event_id: crypto.randomUUID(),
            device_id: "local_mac_importer",
            timestamp: Date.now(),
            event: {
                type: "BookmarkAdded",
                payload: {
                    id: node.id,
                    url: node.url,
                    title: node.name,
                    host: host,
                    created_at: new Date(parseInt(node.date_added) / 1000).toISOString() // Chrome epoch micro to ISO
                }
            }
        });
    }

    if (node.children) {
        node.children.forEach(child => traverseBookmarks(child, results));
    }
    return results;
}

function run() {
    if (!fs.existsSync(CHROME_BOOKMARKS_PATH)) {
        console.error("❌ Chrome Bookmarks file not found at:", CHROME_BOOKMARKS_PATH);
        process.exit(1);
    }

    console.log("Found Chrome Bookmarks, parsing...");
    const data = JSON.parse(fs.readFileSync(CHROME_BOOKMARKS_PATH, 'utf-8'));

    const results = [];
    if (data.roots.bookmark_bar) traverseBookmarks(data.roots.bookmark_bar, results);
    if (data.roots.other) traverseBookmarks(data.roots.other, results);
    if (data.roots.synced) traverseBookmarks(data.roots.synced, results);

    console.log(`\nSuccessfully extracted ${results.length} bookmarks.\n`);

    // Dump top 2 for preview
    console.log("Sample Events:");
    console.log(JSON.stringify(results.slice(0, 2), null, 2));

    const outPath = path.join(__dirname, '..', 'initial_import.json');
    fs.writeFileSync(outPath, JSON.stringify(results, null, 2));
    console.log(`\n✅ Full payload dumped to: ${outPath}`);
}

run();

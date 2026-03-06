use crate::events::models::EventLog;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const MAX_SEGMENT_BYTES: u64 = 500 * 1024; // 500 KB
pub const MAX_SEGMENT_LINES: usize = 1000;
pub const CURRENT_SEGMENT_NAME: &str = "events-current.ndjson";
pub const LEGACY_SEGMENT_NAME: &str = "events.ndjson";

/// 追加事件到 events-current.ndjson；若超限则先封口再追加
pub fn append_to_current_segment(events_dir: &Path, logs: &[EventLog]) -> Result<(), String> {
    if logs.is_empty() {
        return Ok(());
    }
    seal_current_if_needed(events_dir)?;
    let current = events_dir.join(CURRENT_SEGMENT_NAME);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&current)
        .map_err(|e| e.to_string())?;
    for log in logs {
        let line = serde_json::to_string(log).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 检查 events-current.ndjson 大小/行数，若超限则封口为 events-XXXXXX.ndjson
pub fn seal_current_if_needed(events_dir: &Path) -> Result<(), String> {
    let current = events_dir.join(CURRENT_SEGMENT_NAME);
    if !current.exists() {
        return Ok(());
    }
    let meta = fs::metadata(&current).map_err(|e| e.to_string())?;
    let over_size = meta.len() >= MAX_SEGMENT_BYTES;
    let over_lines = count_lines(&current)? >= MAX_SEGMENT_LINES;
    if over_size || over_lines {
        let sealed_name = next_sealed_segment_name(events_dir)?;
        let sealed_path = events_dir.join(&sealed_name);
        fs::rename(&current, &sealed_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 返回 events_dir 中所有封口的 segment 文件（按文件名排序，不含 current）
pub fn list_sealed_segments(events_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut segments: Vec<PathBuf> = fs::read_dir(events_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("events-") && n.ends_with(".ndjson") && n != CURRENT_SEGMENT_NAME)
                .unwrap_or(false)
        })
        .collect();
    segments.sort();
    Ok(segments)
}

/// 遍历所有 segment（sealed + current）按 timestamp 升序返回所有事件
pub fn read_all_events(events_dir: &Path) -> Result<Vec<EventLog>, String> {
    let mut all_events: Vec<EventLog> = Vec::new();

    // 读取所有 sealed segments
    let mut files = list_sealed_segments(events_dir)?;

    // 加入 current（如果存在）
    let current = events_dir.join(CURRENT_SEGMENT_NAME);
    if current.exists() {
        files.push(current);
    }

    for file in &files {
        let mut evts = read_events_from_file(file)?;
        all_events.append(&mut evts);
    }

    // 按 timestamp 升序排序
    all_events.sort_by_key(|e| e.timestamp);
    Ok(all_events)
}

/// GitHub 文件大小限制：100MB，留 10MB 裕量
pub const MAX_GITHUB_FILE_BYTES: u64 = 90 * 1024 * 1024; // 90 MB

/// 将旧版 events.ndjson 迁移为一个或多个 segment 文件（向后兼容）
/// 若文件超过 90MB，会自动拆分为多个 segment
pub fn migrate_legacy_if_exists(events_dir: &Path) -> Result<(), String> {
    let legacy = events_dir.join(LEGACY_SEGMENT_NAME);
    if !legacy.exists() {
        return Ok(());
    }

    let meta = fs::metadata(&legacy).map_err(|e| e.to_string())?;
    if meta.len() <= MAX_GITHUB_FILE_BYTES {
        // 文件较小，直接 rename
        let target = events_dir.join("events-000001.ndjson");
        if !target.exists() {
            fs::rename(&legacy, &target).map_err(|e| e.to_string())?;
        } else {
            let content = fs::read(&legacy).map_err(|e| e.to_string())?;
            let mut f = OpenOptions::new().append(true).open(&target).map_err(|e| e.to_string())?;
            f.write_all(&content).map_err(|e| e.to_string())?;
            fs::remove_file(&legacy).map_err(|e| e.to_string())?;
        }
    } else {
        // 文件超过 90MB，按行拆分为多个 segment
        split_legacy_into_segments(events_dir, &legacy)?;
        fs::remove_file(&legacy).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 将大文件按行拆分为多个 ≤ 90MB 的 segment 文件
fn split_legacy_into_segments(events_dir: &Path, legacy: &Path) -> Result<(), String> {
    use std::io::{BufReader, BufRead};
    let f = File::open(legacy).map_err(|e| e.to_string())?;
    let reader = BufReader::new(f);

    let mut seg_num = list_sealed_segments(events_dir)?.len() + 1;
    let mut current_path = events_dir.join(format!("events-{:06}.ndjson", seg_num));
    let mut current_file = OpenOptions::new()
        .create(true).append(true)
        .open(&current_path)
        .map_err(|e| e.to_string())?;
    let mut current_size: u64 = 0;

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() { continue; }

        let bytes = line.as_bytes().len() as u64 + 1; // +1 for newline
        if current_size + bytes > MAX_GITHUB_FILE_BYTES && current_size > 0 {
            // 当前 segment 已满，开新文件
            seg_num += 1;
            current_path = events_dir.join(format!("events-{:06}.ndjson", seg_num));
            current_file = OpenOptions::new()
                .create(true).append(true)
                .open(&current_path)
                .map_err(|e| e.to_string())?;
            current_size = 0;
        }
        writeln!(current_file, "{}", line).map_err(|e| e.to_string())?;
        current_size += bytes;
    }
    Ok(())
}


fn next_sealed_segment_name(events_dir: &Path) -> Result<String, String> {
    let existing = list_sealed_segments(events_dir)?;
    let next_num = existing.len() + 1;
    Ok(format!("events-{:06}.ndjson", next_num))
}

fn count_lines(path: &Path) -> Result<usize, String> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(f);
    let count = reader
        .lines()
        .filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false))
        .count();
    Ok(count)
}

pub fn read_events_from_file(path: &Path) -> Result<Vec<EventLog>, String> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(f);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let log: EventLog = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        events.push(log);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::models::{EventLog, SyncEvent};
    use tempfile::TempDir;

    fn make_event(timestamp: i64) -> EventLog {
        EventLog {
            event_id: uuid::Uuid::new_v4().to_string(),
            device_id: "test-device".to_string(),
            timestamp,
            event: SyncEvent::BookmarkDeleted {
                id: "bm-1".to_string(),
            },
        }
    }

    fn make_events_dir() -> TempDir {
        tempfile::tempdir().expect("create tempdir")
    }

    // ── RED 1: current 文件行数超限时应自动封口 ──────────────────────────────
    #[test]
    fn segment_seals_when_over_line_limit() {
        let dir = make_events_dir();
        let events_dir = dir.path();

        // 写入 MAX_SEGMENT_LINES 条事件
        let batch: Vec<EventLog> = (0..MAX_SEGMENT_LINES as i64)
            .map(|i| make_event(1_000_000 + i))
            .collect();
        append_to_current_segment(events_dir, &batch).expect("first append");

        // 此时 current 有 1000 条，再追加 1 条应触发封口
        let one_more = vec![make_event(2_000_000)];
        append_to_current_segment(events_dir, &one_more).expect("second append");

        // 应该出现一个封口的 segment 文件
        let sealed = list_sealed_segments(events_dir).expect("list sealed");
        assert_eq!(
            sealed.len(),
            1,
            "should have exactly 1 sealed segment after overflow"
        );

        // sealed segment 应包含原来的 1000 条
        let sealed_events = read_events_from_file(&sealed[0]).expect("read sealed");
        assert_eq!(sealed_events.len(), MAX_SEGMENT_LINES);

        // current 应该只有 1 条
        let current_path = events_dir.join(CURRENT_SEGMENT_NAME);
        let current_count = count_lines(&current_path).expect("count current lines");
        assert_eq!(current_count, 1);
    }

    // ── RED 2: current 文件大小超限时应自动封口 ──────────────────────────────
    #[test]
    fn segment_seals_when_over_size_limit() {
        let dir = make_events_dir();
        let events_dir = dir.path();

        // 构造一个足够大的事件（通过大 id 填充），确保 < 1000 条就能超过 500KB
        let big_event = EventLog {
            event_id: uuid::Uuid::new_v4().to_string(),
            device_id: "test-device".to_string(),
            timestamp: 1_000_000,
            event: SyncEvent::BookmarkDeleted {
                id: "a".repeat(600), // 600 字节的 id，每条 ~700 字节
            },
        };
        // 写入约 750 条（≈ 525KB > 500KB）
        let batch: Vec<EventLog> = (0..750)
            .map(|i| {
                let mut e = big_event.clone();
                e.timestamp = 1_000_000 + i;
                e
            })
            .collect();
        append_to_current_segment(events_dir, &batch).expect("append big batch");

        // 再追加 1 条来触发检查
        let one_more = vec![make_event(9_000_000)];
        append_to_current_segment(events_dir, &one_more).expect("trigger seal");

        let sealed = list_sealed_segments(events_dir).expect("list sealed");
        assert!(
            !sealed.is_empty(),
            "should have at least 1 sealed segment after size overflow"
        );
    }

    // ── RED 3: read_all_events 应按 timestamp 升序跨多个 segment 返回 ─────────
    #[test]
    fn read_all_events_returns_sorted_by_timestamp() {
        let dir = make_events_dir();
        let events_dir = dir.path();

        let seg1_path = events_dir.join("events-000001.ndjson");
        let seg2_path = events_dir.join("events-000002.ndjson");
        let current_path = events_dir.join(CURRENT_SEGMENT_NAME);

        let e1 = make_event(1000);
        let e2 = make_event(3000);
        let e3 = make_event(2000);

        write_events_to_file(&seg1_path, &[e1]).expect("write seg1");
        write_events_to_file(&seg2_path, &[e2]).expect("write seg2");
        write_events_to_file(&current_path, &[e3]).expect("write current");

        let all = read_all_events(events_dir).expect("read all");
        assert_eq!(all.len(), 3);
        assert!(
            all[0].timestamp <= all[1].timestamp && all[1].timestamp <= all[2].timestamp,
            "events should be sorted by timestamp: {:?}",
            all.iter().map(|e| e.timestamp).collect::<Vec<_>>()
        );
    }

    // ── RED 4: 旧版 events.ndjson 应被自动迁移为 events-000001.ndjson ─────────
    #[test]
    fn legacy_single_file_migrated_on_first_read() {
        let dir = make_events_dir();
        let events_dir = dir.path();

        // 模拟旧版：存在 events.ndjson
        let legacy_path = events_dir.join(LEGACY_SEGMENT_NAME);
        let e = make_event(1000);
        write_events_to_file(&legacy_path, &[e]).expect("write legacy");

        migrate_legacy_if_exists(events_dir).expect("migrate");

        // 旧文件应不存在
        assert!(
            !legacy_path.exists(),
            "legacy events.ndjson should be removed after migration"
        );
        // 新文件应存在
        let new_path = events_dir.join("events-000001.ndjson");
        assert!(
            new_path.exists(),
            "events-000001.ndjson should exist after migration"
        );

        // 内容应完整
        let events = read_events_from_file(&new_path).expect("read migrated");
        assert_eq!(events.len(), 1);
    }

    // 测试辅助：写事件到指定文件
    fn write_events_to_file(path: &Path, logs: &[EventLog]) -> Result<(), String> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        for log in logs {
            let line = serde_json::to_string(log).map_err(|e| e.to_string())?;
            writeln!(f, "{line}").map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

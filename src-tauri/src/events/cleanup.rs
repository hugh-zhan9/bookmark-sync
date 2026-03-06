use crate::events::device_registry::min_synced_ts_across_devices;
use crate::events::segment::{list_sealed_segments, read_events_from_file};
use std::fs;
use std::path::Path;

/// 尝试删除所有活跃设备均已同步过的旧 segment 文件
/// 返回成功删除的文件数量
pub fn try_cleanup_old_segments(events_dir: &Path, devices_dir: &Path) -> Result<usize, String> {
    // 若无活跃设备记录，保守起见不做清理
    let min_ts = match min_synced_ts_across_devices(devices_dir)? {
        Some(ts) => ts,
        None => return Ok(0),
    };

    let sealed = list_sealed_segments(events_dir)?;
    let mut deleted = 0;

    for segment_path in &sealed {
        let events = read_events_from_file(segment_path)?;
        // segment 为空或所有事件的最大 timestamp < min_ts → 可安全删除
        let max_ts = events.iter().map(|e| e.timestamp).max().unwrap_or(i64::MIN);
        if max_ts < min_ts {
            fs::remove_file(segment_path).map_err(|e| e.to_string())?;
            deleted += 1;
        }
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::device_registry::update_device;
    use crate::events::models::{EventLog, SyncEvent};
    use crate::events::segment::append_to_current_segment;
    use std::fs::OpenOptions;
    use std::io::Write;
    use tempfile::TempDir;

    struct TestDirs {
        _tmp: TempDir,
        pub events_dir: std::path::PathBuf,
        pub devices_dir: std::path::PathBuf,
    }

    fn setup() -> TestDirs {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_dir = tmp.path().join("events");
        let devices_dir = tmp.path().join("devices");
        fs::create_dir_all(&events_dir).unwrap();
        fs::create_dir_all(&devices_dir).unwrap();
        TestDirs { _tmp: tmp, events_dir, devices_dir }
    }

    fn make_event(timestamp: i64) -> EventLog {
        EventLog {
            event_id: uuid::Uuid::new_v4().to_string(),
            device_id: "test-device".to_string(),
            timestamp,
            event: SyncEvent::BookmarkDeleted { id: "b1".to_string() },
        }
    }

    fn write_sealed_segment(events_dir: &Path, name: &str, events: &[EventLog]) {
        let path = events_dir.join(name);
        let mut f = OpenOptions::new().create(true).append(true).open(&path).unwrap();
        for e in events {
            writeln!(f, "{}", serde_json::to_string(e).unwrap()).unwrap();
        }
    }

    // ── RED 1: 所有设备都已同步了某 segment → 应删除该 segment ──────────────
    #[test]
    fn cleanup_deletes_fully_synced_segment() {
        let ctx = setup();

        // segment 中最大 timestamp = 1000
        write_sealed_segment(&ctx.events_dir, "events-000001.ndjson", &[
            make_event(500),
            make_event(1000),
        ]);

        // 两台设备都同步到了 2000（> 1000）
        update_device(&ctx.devices_dir, "device-a", 2000).unwrap();
        update_device(&ctx.devices_dir, "device-b", 2000).unwrap();

        let deleted = try_cleanup_old_segments(&ctx.events_dir, &ctx.devices_dir)
            .expect("cleanup");
        assert_eq!(deleted, 1, "should delete 1 segment");
        assert!(
            !ctx.events_dir.join("events-000001.ndjson").exists(),
            "segment file should be gone"
        );
    }

    // ── RED 2: 某设备未同步完 segment → 不删除 ──────────────────────────────
    #[test]
    fn cleanup_keeps_segment_if_one_device_behind() {
        let ctx = setup();

        // segment 中最大 timestamp = 1000
        write_sealed_segment(&ctx.events_dir, "events-000001.ndjson", &[
            make_event(500),
            make_event(1000),
        ]);

        // device-a 同步到 2000，device-b 只同步到 800（< 1000）
        update_device(&ctx.devices_dir, "device-a", 2000).unwrap();
        update_device(&ctx.devices_dir, "device-b", 800).unwrap();

        let deleted = try_cleanup_old_segments(&ctx.events_dir, &ctx.devices_dir)
            .expect("cleanup");
        assert_eq!(deleted, 0, "should not delete any segment");
        assert!(
            ctx.events_dir.join("events-000001.ndjson").exists(),
            "segment file should still exist"
        );
    }

    // ── 边界：无设备记录时不清理 ─────────────────────────────────────────────
    #[test]
    fn cleanup_does_nothing_when_no_devices() {
        let ctx = setup();

        write_sealed_segment(&ctx.events_dir, "events-000001.ndjson", &[make_event(500)]);

        let deleted = try_cleanup_old_segments(&ctx.events_dir, &ctx.devices_dir)
            .expect("cleanup");
        assert_eq!(deleted, 0, "should not delete anything without device registry");
    }

    // ── 多 segment 部分可清理场景 ────────────────────────────────────────────
    #[test]
    fn cleanup_deletes_only_eligible_segments() {
        let ctx = setup();

        // seg1: max_ts = 1000，seg2: max_ts = 3000
        write_sealed_segment(&ctx.events_dir, "events-000001.ndjson", &[make_event(1000)]);
        write_sealed_segment(&ctx.events_dir, "events-000002.ndjson", &[make_event(3000)]);

        // 设备同步到 2000：seg1 可清理，seg2 不可清理
        update_device(&ctx.devices_dir, "device-a", 2000).unwrap();

        let deleted = try_cleanup_old_segments(&ctx.events_dir, &ctx.devices_dir)
            .expect("cleanup");
        assert_eq!(deleted, 1, "should delete only seg1");
        assert!(!ctx.events_dir.join("events-000001.ndjson").exists());
        assert!(ctx.events_dir.join("events-000002.ndjson").exists());
    }
}

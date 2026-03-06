use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub const DEVICE_TTL_DAYS: i64 = 60;
const MS_PER_DAY: i64 = 86_400_000;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceRecord {
    /// 该设备已成功同步的最大事件 timestamp（ms）
    pub last_synced_ts: i64,
    /// 该设备上次更新注册记录的时间（ms）
    pub last_seen_ts: i64,
}

/// 写入/更新当前设备的注册记录
pub fn update_device(devices_dir: &Path, device_id: &str, last_synced_ts: i64) -> Result<(), String> {
    fs::create_dir_all(devices_dir).map_err(|e| e.to_string())?;
    let now_ts = chrono::Utc::now().timestamp_millis();
    let record = DeviceRecord {
        last_synced_ts,
        last_seen_ts: now_ts,
    };
    let path = devices_dir.join(format!("{}.json", device_id));
    let json = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    f.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

/// 读取所有活跃设备（过滤掉超过 DEVICE_TTL_DAYS 天未见的设备）
pub fn read_active_devices(devices_dir: &Path) -> Result<HashMap<String, DeviceRecord>, String> {
    if !devices_dir.exists() {
        return Ok(HashMap::new());
    }
    let now_ts = chrono::Utc::now().timestamp_millis();
    let ttl_ms = DEVICE_TTL_DAYS * MS_PER_DAY;
    let mut result = HashMap::new();

    for entry in fs::read_dir(devices_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let device_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| "invalid device file name".to_string())?
            .to_string();

        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let record: DeviceRecord = serde_json::from_str(&content).map_err(|e| e.to_string())?;

        // 跳过超过 TTL 的设备
        if now_ts - record.last_seen_ts > ttl_ms {
            continue;
        }
        result.insert(device_id, record);
    }
    Ok(result)
}

/// 返回所有活跃设备中 last_synced_ts 的最小值（清理安全线）
/// 返回 None 表示没有活跃设备（此时不应清理）
pub fn min_synced_ts_across_devices(devices_dir: &Path) -> Result<Option<i64>, String> {
    let devices = read_active_devices(devices_dir)?;
    if devices.is_empty() {
        return Ok(None);
    }
    let min_ts = devices.values().map(|r| r.last_synced_ts).min();
    Ok(min_ts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_devices_dir() -> TempDir {
        tempfile::tempdir().expect("create tempdir")
    }

    // ── RED 1: 写入设备记录后应能正确读取回来 ────────────────────────────────
    #[test]
    fn device_registry_update_and_read_device() {
        let dir = make_devices_dir();
        let devices_dir = dir.path();

        update_device(devices_dir, "mac-home", 1_000_000).expect("update device");

        let devices = read_active_devices(devices_dir).expect("read devices");
        assert!(devices.contains_key("mac-home"), "mac-home should be in registry");
        let record = &devices["mac-home"];
        assert_eq!(record.last_synced_ts, 1_000_000);
        assert!(record.last_seen_ts > 0);
    }

    // ── RED 2: 超过 TTL 的设备应被过滤掉 ───────────────────────────────────
    #[test]
    fn device_registry_excludes_expired_device() {
        let dir = make_devices_dir();
        let devices_dir = dir.path();

        // 手动写一个 last_seen_ts 为 61 天前的设备记录
        let now_ts = chrono::Utc::now().timestamp_millis();
        let expired_ts = now_ts - (61 * 86_400_000_i64);
        let expired_record = DeviceRecord {
            last_synced_ts: 500_000,
            last_seen_ts: expired_ts,
        };
        fs::create_dir_all(devices_dir).unwrap();
        let path = devices_dir.join("old-device.json");
        fs::write(&path, serde_json::to_string(&expired_record).unwrap()).unwrap();

        // 同时写一个活跃设备
        update_device(devices_dir, "active-device", 1_000_000).expect("update active device");

        let devices = read_active_devices(devices_dir).expect("read devices");
        assert!(!devices.contains_key("old-device"), "expired device should be excluded");
        assert!(devices.contains_key("active-device"), "active device should be included");
        assert_eq!(devices.len(), 1);
    }

    // ── RED 3: min_synced_ts 应返回所有活跃设备中最小的同步时间戳 ──────────
    #[test]
    fn min_synced_ts_returns_minimum_across_active_devices() {
        let dir = make_devices_dir();
        let devices_dir = dir.path();

        update_device(devices_dir, "device-a", 5_000_000).expect("update a");
        update_device(devices_dir, "device-b", 2_000_000).expect("update b");
        update_device(devices_dir, "device-c", 8_000_000).expect("update c");

        let min_ts = min_synced_ts_across_devices(devices_dir)
            .expect("min synced ts")
            .expect("should have a value");
        assert_eq!(min_ts, 2_000_000, "min should be device-b's timestamp");
    }

    // ── 边界：设备目录为空时 min_synced_ts 返回 None ─────────────────────────
    #[test]
    fn min_synced_ts_returns_none_when_no_devices() {
        let dir = make_devices_dir();
        let devices_dir = dir.path().join("empty-devices");

        let result = min_synced_ts_across_devices(&devices_dir).expect("ok");
        assert!(result.is_none(), "should return None when no devices");
    }
}

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use tempfile::NamedTempFile;

pub fn atomic_write_bytes(target: &Path, data: &[u8]) -> std::io::Result<()> {
    let mut tmp = atomic_write_file(target)?;
    tmp.write_all(data)?;
    persist_tempfile(target, tmp)
}

pub fn atomic_write_string(target: &Path, data: &str) -> std::io::Result<()> {
    atomic_write_bytes(target, data.as_bytes())
}

pub fn atomic_write_file(target: &Path) -> std::io::Result<NamedTempFile> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
        NamedTempFile::new_in(parent)
    } else {
        NamedTempFile::new()
    }
}

pub fn persist_tempfile(target: &Path, mut file: NamedTempFile) -> std::io::Result<()> {
    file.as_file_mut().flush()?;
    file.as_file_mut().sync_all()?;
    match file.persist(target) {
        Ok(_) => Ok(()),
        Err(err) => Err(err.error),
    }
}

pub fn read_to_string_if_exists(path: &Path) -> std::io::Result<Option<String>> {
    if path.exists() {
        std::fs::read_to_string(path).map(Some)
    } else {
        Ok(None)
    }
}

pub fn read_json_map(path: &Path) -> Map<String, Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

pub fn write_json_pretty(path: &Path, value: &serde_json::Value) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    atomic_write_bytes(path, &bytes)
}

pub fn write_toml_pretty(path: &Path, value: &toml::Value) -> std::io::Result<()> {
    let output = toml::to_string_pretty(value).map_err(std::io::Error::other)?;
    atomic_write_string(path, &(output + "\n"))
}

pub fn create_file(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    File::create(path)
}

/// Current time as a Unix timestamp string (seconds since epoch).
pub fn now_unix_secs() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

pub fn read_toml_table(path: &Path) -> toml::Table {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| toml::from_str::<toml::Value>(&content).ok())
        .and_then(|value| value.as_table().cloned())
        .unwrap_or_default()
}

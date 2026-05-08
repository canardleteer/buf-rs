//! Cache-slot lock helpers for coordinating concurrent build.rs writers.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOCK_FILENAME: &str = ".cache-slot.lock";
const LOCK_WAIT_STEP: Duration = Duration::from_millis(250);
const LOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const LOCK_STALE_AFTER: Duration = Duration::from_secs(600);

pub enum SlotLockState {
    Acquired(SlotLockGuard),
    WaitedForOtherWriter,
}

pub struct SlotLockGuard {
    path: PathBuf,
}

impl Drop for SlotLockGuard {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

pub fn acquire_or_wait_for_slot(
    slot: &Path,
    warn: &mut dyn FnMut(String),
) -> Result<SlotLockState, String> {
    let lock_path = slot.join(LOCK_FILENAME);
    match try_acquire(&lock_path) {
        Ok(Some(guard)) => Ok(SlotLockState::Acquired(guard)),
        Ok(None) => {
            warn(format!(
                "buf-tools: cache slot lock exists at {}. Waiting for peer writer.",
                lock_path.display()
            ));
            wait_for_unlock(&lock_path, warn)?;
            Ok(SlotLockState::WaitedForOtherWriter)
        }
        Err(err) => Err(err),
    }
}

fn try_acquire(lock_path: &Path) -> Result<Option<SlotLockGuard>, String> {
    let mut lock = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => return Ok(None),
        Err(err) => return Err(format!("create lock {}: {err}", lock_path.display())),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system clock before unix epoch: {e}"))?
        .as_secs();
    let pid = std::process::id();
    writeln!(lock, "pid={pid} unix_ts={now}")
        .map_err(|e| format!("write lock {}: {e}", lock_path.display()))?;
    Ok(Some(SlotLockGuard {
        path: lock_path.to_path_buf(),
    }))
}

fn wait_for_unlock(lock_path: &Path, warn: &mut dyn FnMut(String)) -> Result<(), String> {
    let mut waited = Duration::ZERO;
    while lock_path.exists() {
        if is_stale(lock_path)? {
            warn(format!(
                "buf-tools: cache slot lock looks stale at {}. Removing stale lock.",
                lock_path.display()
            ));
            fs::remove_file(lock_path)
                .map_err(|e| format!("remove stale lock {}: {e}", lock_path.display()))?;
            return Ok(());
        }
        if waited >= LOCK_WAIT_TIMEOUT {
            return Err(format!(
                "buf-tools: timed out waiting for cache slot lock {} after {}s",
                lock_path.display(),
                LOCK_WAIT_TIMEOUT.as_secs()
            ));
        }
        sleep(LOCK_WAIT_STEP);
        waited += LOCK_WAIT_STEP;
    }
    warn(format!(
        "buf-tools: peer writer released cache slot lock {} after {}ms",
        lock_path.display(),
        waited.as_millis()
    ));
    Ok(())
}

fn is_stale(lock_path: &Path) -> Result<bool, String> {
    if let Some(owner_pid) = read_lock_pid(lock_path)?
        && !pid_is_alive(owner_pid)
    {
        return Ok(true);
    }
    let meta = fs::metadata(lock_path).map_err(|e| format!("stat {}: {e}", lock_path.display()))?;
    let modified = meta
        .modified()
        .map_err(|e| format!("mtime {}: {e}", lock_path.display()))?;
    let age = SystemTime::now()
        .duration_since(modified)
        .map_err(|e| format!("mtime in future for {}: {e}", lock_path.display()))?;
    Ok(age >= LOCK_STALE_AFTER)
}

fn read_lock_pid(lock_path: &Path) -> Result<Option<u32>, String> {
    let content =
        fs::read_to_string(lock_path).map_err(|e| format!("read {}: {e}", lock_path.display()))?;
    let Some(pid_part) = content
        .split_whitespace()
        .find(|part| part.starts_with("pid="))
    else {
        return Ok(None);
    };
    let pid = pid_part["pid=".len()..].parse::<u32>().ok();
    Ok(pid)
}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    PathBuf::from("/proc").join(pid.to_string()).exists()
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn test_dir(name: &str) -> PathBuf {
        let unique = format!(
            "buf-tools-lock-test-{}-{}",
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn acquires_and_releases_lock() {
        let slot = test_dir("acquire-release");
        let mut warn = |_msg: String| {};
        let state = acquire_or_wait_for_slot(&slot, &mut warn).expect("acquire");
        let guard = match state {
            SlotLockState::Acquired(guard) => guard,
            SlotLockState::WaitedForOtherWriter => panic!("unexpected wait"),
        };
        assert!(slot.join(LOCK_FILENAME).exists());
        drop(guard);
        assert!(!slot.join(LOCK_FILENAME).exists());
    }

    #[test]
    fn waits_for_peer_then_returns_waited_state() {
        let slot = test_dir("wait-peer");
        let lock_path = slot.join(LOCK_FILENAME);
        let mut lock = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path)
            .expect("create peer lock");
        writeln!(lock, "peer lock").expect("write peer lock");
        drop(lock);

        let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let logs_for_thread = Arc::clone(&logs);
        let slot_for_thread = slot.clone();
        let handle = thread::spawn(move || {
            let mut warn = |msg: String| logs_for_thread.lock().expect("lock logs").push(msg);
            acquire_or_wait_for_slot(&slot_for_thread, &mut warn)
        });

        sleep(Duration::from_millis(300));
        fs::remove_file(&lock_path).expect("release peer lock");

        let state = handle.join().expect("thread join").expect("wait state");
        assert!(matches!(state, SlotLockState::WaitedForOtherWriter));
        let joined = logs.lock().expect("lock logs").join("\n");
        assert!(
            joined.contains("Waiting for peer writer")
                && joined.contains("released cache slot lock"),
            "logs were: {joined}"
        );
    }
}

//! Cache-slot lock helpers for coordinating concurrent build.rs writers.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOCK_FILENAME: &str = ".cache-slot.lock";
const LOCK_WAIT_STEP: Duration = Duration::from_millis(250);
const LOCK_WAIT_TIMEOUT_ENV: &str = "BUF_RS_CACHE_LOCK_TIMEOUT_SECS";
const DEFAULT_LOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
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
    let timeout = parse_lock_wait_timeout(std::env::var_os(LOCK_WAIT_TIMEOUT_ENV).as_deref())?;
    let lock_path = slot.join(LOCK_FILENAME);
    match try_acquire(&lock_path) {
        Ok(Some(guard)) => Ok(SlotLockState::Acquired(guard)),
        Ok(None) => {
            warn(format!(
                "buf-tools: cache slot lock exists at {}. Waiting for peer writer (timeout: {}s).",
                lock_path.display(),
                timeout.as_secs()
            ));
            wait_for_unlock(&lock_path, timeout, warn)?;
            Ok(SlotLockState::WaitedForOtherWriter)
        }
        Err(err) => Err(err),
    }
}

fn parse_lock_wait_timeout(value: Option<&OsStr>) -> Result<Duration, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_LOCK_WAIT_TIMEOUT);
    };
    let seconds = value
        .to_str()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .ok_or_else(|| {
            format!(
                "buf-tools: invalid {LOCK_WAIT_TIMEOUT_ENV}={value:?}; expected a positive whole number of seconds (default: {}s)",
                DEFAULT_LOCK_WAIT_TIMEOUT.as_secs()
            )
        })?;
    Ok(Duration::from_secs(seconds))
}

fn try_acquire(lock_path: &Path) -> Result<Option<SlotLockGuard>, String> {
    let mut lock = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::AlreadyExists => return Ok(None),
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

fn wait_for_unlock(
    lock_path: &Path,
    timeout: Duration,
    warn: &mut dyn FnMut(String),
) -> Result<(), String> {
    let mut waited = Duration::ZERO;
    loop {
        let mut lock = match File::open(lock_path) {
            Ok(lock) => lock,
            Err(err) if err.kind() == ErrorKind::NotFound => break,
            Err(err) => return Err(format!("open lock {}: {err}", lock_path.display())),
        };
        // The peer may unlink its lock at any point. Read the PID and metadata
        // through the same handle so release cannot turn inspection into ENOENT.
        let stale = is_stale(&mut lock, lock_path)?;
        drop(lock);
        if stale {
            warn(format!(
                "buf-tools: cache slot lock looks stale at {}. Removing stale lock.",
                lock_path.display()
            ));
            match fs::remove_file(lock_path) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(format!("remove stale lock {}: {err}", lock_path.display()));
                }
            }
            return Ok(());
        }
        if waited >= timeout {
            return Err(format!(
                "buf-tools: timed out waiting for cache slot lock {} after {}s; set {LOCK_WAIT_TIMEOUT_ENV} to a larger number of seconds for slow downloads",
                lock_path.display(),
                timeout.as_secs()
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

fn is_stale(lock: &mut File, lock_path: &Path) -> Result<bool, String> {
    if let Some(owner_pid) = read_lock_pid(lock, lock_path)?
        && !pid_is_alive(owner_pid)
    {
        return Ok(true);
    }
    let meta = lock
        .metadata()
        .map_err(|e| format!("stat {}: {e}", lock_path.display()))?;
    let modified = meta
        .modified()
        .map_err(|e| format!("mtime {}: {e}", lock_path.display()))?;
    let age = SystemTime::now()
        .duration_since(modified)
        .map_err(|e| format!("mtime in future for {}: {e}", lock_path.display()))?;
    Ok(age >= LOCK_STALE_AFTER)
}

fn read_lock_pid(lock: &mut File, lock_path: &Path) -> Result<Option<u32>, String> {
    let mut content = String::new();
    lock.read_to_string(&mut content)
        .map_err(|e| format!("read {}: {e}", lock_path.display()))?;
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
    use std::sync::mpsc;
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
        fs::remove_dir_all(&slot).expect("cleanup");
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

        let (waiting, wait_started) = mpsc::channel();
        let slot_for_thread = slot.clone();
        let handle = thread::spawn(move || {
            let mut logs = Vec::new();
            let mut warn = |msg: String| {
                if msg.contains("Waiting for peer writer") {
                    waiting.send(()).expect("signal waiter");
                }
                logs.push(msg);
            };
            let state = acquire_or_wait_for_slot(&slot_for_thread, &mut warn);
            (state, logs)
        });

        wait_started
            .recv_timeout(Duration::from_secs(10))
            .expect("waiter started");
        fs::remove_file(&lock_path).expect("release peer lock");

        let (state, logs) = handle.join().expect("thread join");
        let state = state.expect("wait state");
        assert!(matches!(state, SlotLockState::WaitedForOtherWriter));
        let joined = logs.join("\n");
        assert!(
            joined.contains("Waiting for peer writer")
                && joined.contains("released cache slot lock"),
            "logs were: {joined}"
        );
        fs::remove_dir_all(&slot).expect("cleanup");
    }

    #[test]
    fn peer_release_before_inspection_returns_waited_state() {
        let slot = test_dir("release-before-inspection");
        let lock_path = slot.join(LOCK_FILENAME);
        fs::write(&lock_path, "peer lock").expect("write peer lock");
        let mut warn = |msg: String| {
            if msg.contains("Waiting for peer writer") {
                fs::remove_file(&lock_path).expect("release peer lock");
            }
        };

        let state = acquire_or_wait_for_slot(&slot, &mut warn).expect("wait state");
        assert!(matches!(state, SlotLockState::WaitedForOtherWriter));
        assert!(!lock_path.exists());
        fs::remove_dir_all(&slot).expect("cleanup");
    }

    #[test]
    fn inspects_open_lock_after_peer_unlinks_it() {
        let slot = test_dir("release-during-inspection");
        let lock_path = slot.join(LOCK_FILENAME);
        // No PID forces inspection to read the file's metadata too.
        fs::write(&lock_path, "peer lock").expect("write peer lock");
        let mut lock = File::open(&lock_path).expect("open peer lock");
        fs::remove_file(&lock_path).expect("release peer lock");

        assert!(!is_stale(&mut lock, &lock_path).expect("inspect released lock"));
        drop(lock);
        wait_for_unlock(&lock_path, DEFAULT_LOCK_WAIT_TIMEOUT, &mut |_| {})
            .expect("observe release");
        fs::remove_dir_all(&slot).expect("cleanup");
    }

    fn write_stale_lock(lock_path: &Path) {
        fs::write(lock_path, "peer lock").expect("write peer lock");
        File::options()
            .write(true)
            .open(lock_path)
            .expect("open peer lock")
            .set_modified(UNIX_EPOCH)
            .expect("age peer lock");
    }

    #[test]
    fn removes_stale_lock() {
        let slot = test_dir("stale-lock");
        let lock_path = slot.join(LOCK_FILENAME);
        write_stale_lock(&lock_path);

        wait_for_unlock(&lock_path, DEFAULT_LOCK_WAIT_TIMEOUT, &mut |_| {})
            .expect("remove stale lock");
        assert!(!lock_path.exists());
        fs::remove_dir_all(&slot).expect("cleanup");
    }

    #[test]
    fn peer_release_during_stale_cleanup_succeeds() {
        let slot = test_dir("release-during-cleanup");
        let lock_path = slot.join(LOCK_FILENAME);
        write_stale_lock(&lock_path);
        let mut peer_released = false;
        let mut warn = |msg: String| {
            if msg.contains("Removing stale lock") {
                fs::remove_file(&lock_path).expect("release peer lock");
                peer_released = true;
            }
        };

        wait_for_unlock(&lock_path, DEFAULT_LOCK_WAIT_TIMEOUT, &mut warn)
            .expect("already released");
        assert!(peer_released);
        assert!(!lock_path.exists());
        fs::remove_dir_all(&slot).expect("cleanup");
    }

    #[test]
    fn inspection_error_is_not_ignored() {
        let slot = test_dir("inspection-error");
        let lock_path = slot.join(LOCK_FILENAME);
        fs::write(&lock_path, [0xff]).expect("write invalid lock");

        let err = wait_for_unlock(&lock_path, DEFAULT_LOCK_WAIT_TIMEOUT, &mut |_| {})
            .expect_err("invalid UTF-8");
        assert!(err.starts_with("read "), "{err}");
        assert!(
            err.contains(lock_path.to_str().expect("UTF-8 path")),
            "{err}"
        );
        assert!(lock_path.exists());
        fs::remove_dir_all(&slot).expect("cleanup");
    }

    #[test]
    fn stale_removal_error_is_not_ignored() {
        let slot = test_dir("removal-error");
        let lock_path = slot.join(LOCK_FILENAME);
        write_stale_lock(&lock_path);
        let mut warn = |msg: String| {
            if msg.contains("Removing stale lock") {
                fs::remove_file(&lock_path).expect("release peer lock");
                fs::create_dir(&lock_path).expect("replace with directory");
            }
        };

        let err = wait_for_unlock(&lock_path, DEFAULT_LOCK_WAIT_TIMEOUT, &mut warn)
            .expect_err("cannot remove directory");
        assert!(err.starts_with("remove stale lock "), "{err}");
        assert!(
            err.contains(lock_path.to_str().expect("UTF-8 path")),
            "{err}"
        );
        assert!(lock_path.is_dir());
        fs::remove_dir_all(&slot).expect("cleanup");
    }

    #[test]
    fn lock_wait_timeout_defaults_to_120_seconds() {
        assert_eq!(
            parse_lock_wait_timeout(None).expect("default timeout"),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn lock_wait_timeout_accepts_positive_whole_seconds() {
        for (value, seconds) in [
            ("1", 1),
            ("300", 300),
            (" 900 ", 900),
            ("18446744073709551615", u64::MAX),
        ] {
            assert_eq!(
                parse_lock_wait_timeout(Some(OsStr::new(value))).expect("valid timeout"),
                Duration::from_secs(seconds),
                "value: {value:?}"
            );
        }
    }

    #[test]
    fn lock_wait_timeout_rejects_invalid_values() {
        for value in ["", " ", "0", "-1", "1.5", "300s", "18446744073709551616"] {
            let err =
                parse_lock_wait_timeout(Some(OsStr::new(value))).expect_err("invalid timeout");
            assert!(err.contains(LOCK_WAIT_TIMEOUT_ENV), "{err}");
            assert!(err.contains("positive whole number of seconds"), "{err}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn lock_wait_timeout_rejects_non_unicode_values() {
        use std::os::unix::ffi::OsStrExt;

        let err = parse_lock_wait_timeout(Some(OsStr::from_bytes(&[0xff])))
            .expect_err("non-Unicode timeout");
        assert!(err.contains(LOCK_WAIT_TIMEOUT_ENV), "{err}");
    }

    #[test]
    fn configured_timeout_stops_waiting_without_removing_peer_lock() {
        let slot = test_dir("configured-timeout");
        let lock_path = slot.join(LOCK_FILENAME);
        fs::write(&lock_path, "peer lock").expect("write peer lock");

        let err = wait_for_unlock(&lock_path, Duration::ZERO, &mut |_| {})
            .expect_err("configured timeout reached");
        assert!(err.contains("after 0s"), "{err}");
        assert!(err.contains(LOCK_WAIT_TIMEOUT_ENV), "{err}");
        assert!(
            lock_path.is_file(),
            "timing out must retain the peer's lock"
        );
        fs::remove_dir_all(&slot).expect("cleanup");
    }
}

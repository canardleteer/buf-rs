//! HTTP download (anonymous, retried) with optional percent progress.

use std::io::Read;
use std::thread::sleep;
use std::time::Duration;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
const MAX_ATTEMPTS: usize = 5;
const CHUNK: usize = 64 * 1024;

/// Full download into memory (for small files: sha256.txt, .minisig).
pub fn download(url: &str) -> Result<Vec<u8>, String> {
    for attempt in 1..=MAX_ATTEMPTS {
        match download_once(url) {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    return Err(format!(
                        "GET {url} failed after {MAX_ATTEMPTS} attempts: {err}"
                    ));
                }
                let wait_ms = 400_u64 * attempt as u64;
                eprintln!(
                    "  transient download failure (attempt {attempt}/{MAX_ATTEMPTS}) for {url}: {err}; retrying in {wait_ms}ms"
                );
                sleep(Duration::from_millis(wait_ms));
            }
        }
    }
    Err(format!("GET {url}: exhausted retry attempts"))
}

fn download_once(url: &str) -> Result<Vec<u8>, String> {
    ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_vec()
        .map_err(|e| e.to_string())
}

/// Download large blob with `cargo:warning=` progress (≥ 10% bands when Content-Length present).
pub fn download_streaming_with_progress(
    url: &str,
    label: &str,
    warn: &mut dyn FnMut(String),
) -> Result<Vec<u8>, String> {
    for attempt in 1..=MAX_ATTEMPTS {
        match download_streaming_once(url, label, warn) {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    return Err(format!(
                        "GET {url} failed after {MAX_ATTEMPTS} attempts: {err}"
                    ));
                }
                let wait_ms = 400_u64 * attempt as u64;
                warn(format!(
                    "buf-tools: transient download error for {label} (attempt {attempt}/{MAX_ATTEMPTS}): {err}; retry in {wait_ms}ms"
                ));
                sleep(Duration::from_millis(wait_ms));
            }
        }
    }
    Err(format!("GET {url}: exhausted retry attempts"))
}

fn download_streaming_once(
    url: &str,
    label: &str,
    warn: &mut dyn FnMut(String),
) -> Result<Vec<u8>, String> {
    let mut resp = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;

    let total = resp
        .headers()
        .get("Content-Length")
        .and_then(|s| s.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let mut reader = resp.body_mut().as_reader();
    let mut buf = Vec::new();
    let mut read_total: u64 = 0;
    // Last printed milestone percent (0, 10, …, 100); 0% emitted above.
    let mut milestone_sent: i32 = 0;

    let mut chunk = [0u8; CHUNK];
    warn(format!("buf-tools: downloading {label} — 0%"));

    let mut last_mb_milestone: u64 = 0;

    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        read_total += n as u64;

        if let Some(t) = total {
            if t > 0 {
                let pct = ((read_total as f64 / t as f64) * 100.0).floor().min(100.0) as i32;
                let band = (pct / 10) * 10;
                if band > milestone_sent {
                    milestone_sent = band;
                    warn(format!(
                        "buf-tools: {label} — {band}% ({read_total}/{t} bytes)"
                    ));
                }
            }
        } else {
            let mb = read_total / (1024 * 1024);
            if mb > last_mb_milestone && mb > 0 {
                last_mb_milestone = mb;
                warn(format!(
                    "buf-tools: {label} — received ≥ {mb} MiB (no Content-Length)"
                ));
            }
        }
    }

    if let Some(t) = total {
        if milestone_sent < 100 {
            warn(format!(
                "buf-tools: {label} — 100% ({read_total}/{t} bytes)"
            ));
        }
    } else {
        warn(format!(
            "buf-tools: {label} — finished ({read_total} bytes, no Content-Length)"
        ));
    }

    Ok(buf)
}

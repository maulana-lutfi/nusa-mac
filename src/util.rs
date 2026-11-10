use std::path::PathBuf;
use std::process::Command;

pub const APP_NAME: &str = "Nusa";
pub const BUNDLE_ID: &str = "id.nusa.mac";

/// ~/Library/Application Support/Nusa (macOS)
pub fn data_dir() -> PathBuf {
    let d = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME);
    let _ = std::fs::create_dir_all(&d);
    d
}

/// ~/Library/Caches/Nusa (macOS)
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn cache_dir() -> PathBuf {
    let d = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_NAME);
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Menjalankan perintah dan mengembalikan stdout jika sukses.
pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// HTTP GET lewat /usr/bin/curl (selalu ada di macOS).
/// Dipakai supaya binary tidak perlu membawa stack TLS sendiri.
pub fn http_get(url: &str) -> Option<String> {
    run(
        "curl",
        &[
            "-fsSL",
            "--max-time",
            "8",
            "-A",
            "Nusa/0.1 (macOS notch app)",
            url,
        ],
    )
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn http_download(url: &str, dest: &std::path::Path) -> bool {
    let dest = dest.to_string_lossy();
    run(
        "curl",
        &["-fsSL", "--max-time", "8", "-o", dest.as_ref(), url],
    )
    .is_some()
}

pub fn fmt_mmss(secs: f64) -> String {
    let s = secs.max(0.0).round() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn fmt_hm_duration(secs: i64) -> String {
    let secs = secs.max(0);
    let h = secs / 3600;
    let m = (secs % 3600 + 59) / 60;
    if h > 0 {
        format!("{h} j {m} m")
    } else {
        format!("{m} m")
    }
}

pub fn now_ts() -> i64 {
    chrono::Local::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encode() {
        assert_eq!(urlencode("Halo Dunia & co"), "Halo%20Dunia%20%26%20co");
        assert_eq!(fmt_mmss(125.4), "2:05");
        assert_eq!(fmt_hm_duration(3600 + 60 * 12), "1 j 12 m");
    }
}

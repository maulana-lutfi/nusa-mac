//! Lapisan OS. `macos.rs` = implementasi asli (AppKit lewat objc2).
//! `stub.rs` = pengganti untuk development/preview di Linux.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenInfo {
    pub has_notch: bool,
    /// Lebar notch fisik (point). 0 jika tidak ada notch.
    pub notch_w: f32,
    /// Tinggi notch / menu bar (point).
    pub notch_h: f32,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub enum DragPayload {
    Files(Vec<String>),
    Text(String),
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{initial_position, Platform};

#[cfg(not(target_os = "macos"))]
mod stub;
#[cfg(not(target_os = "macos"))]
pub use stub::{initial_position, Platform};

// Di Linux: hanya di-type-check (cargo check --features macos-check).
#[cfg(all(not(target_os = "macos"), feature = "macos-check"))]
#[allow(dead_code, unused_imports)]
mod macos;

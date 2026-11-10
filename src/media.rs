//! Model "now playing" + lirik tersinkron dari LRCLIB.

use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Player {
    Spotify,
    Music,
    /// Tab browser (YouTube, YouTube Music, SoundCloud, Spotify Web, Apple Music Web).
    Browser,
}

impl Player {
    /// Urutan prioritas saat beberapa pemutar aktif bersamaan.
    pub const ALL: [Player; 3] = [Player::Spotify, Player::Music, Player::Browser];
    /// Pemutar yang dibaca lewat notifikasi + AppleScript aplikasinya sendiri.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub const NATIVE: [Player; 2] = [Player::Spotify, Player::Music];
    pub fn name(self) -> &'static str {
        match self {
            Player::Spotify => "Spotify",
            Player::Music => "Apple Music",
            Player::Browser => "Browser",
        }
    }
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn bundle_id(self) -> &'static str {
        match self {
            Player::Spotify => "com.spotify.client",
            Player::Music => "com.apple.Music",
            Player::Browser => "",
        }
    }
    /// Nama aplikasi untuk AppleScript.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn app(self) -> &'static str {
        match self {
            Player::Spotify => "Spotify",
            Player::Music => "Music",
            Player::Browser => "",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub position: f64,
    pub playing: bool,
    /// Label sumber untuk pemutar browser, mis. "YouTube · Chrome". Kosong = nama pemutar.
    pub source: String,
    /// true jika hanya judul yang terbaca (JavaScript dari Apple Events belum diizinkan).
    pub no_control: bool,
}

#[derive(Clone)]
pub struct PlayerState {
    pub track: Track,
    /// Kapan `track.position` diukur — posisi sekarang diekstrapolasi.
    pub at: Instant,
    pub art: Option<Arc<egui::ColorImage>>,
    pub art_key: String,
}

impl PlayerState {
    pub fn position_now(&self) -> f64 {
        let p = if self.track.playing {
            self.track.position + self.at.elapsed().as_secs_f64()
        } else {
            self.track.position
        };
        if self.track.duration > 0.0 {
            p.min(self.track.duration)
        } else {
            p
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaCmd {
    PlayPause,
    Pause,
    Next,
    Prev,
    Seek(f64),
}

#[derive(Clone, Debug, Default)]
pub struct Lyrics {
    /// (detik, teks) terurut.
    pub synced: Vec<(f64, String)>,
    pub plain: Option<String>,
}

impl Lyrics {
    pub fn current_index(&self, pos: f64) -> Option<usize> {
        if self.synced.is_empty() || pos < self.synced[0].0 {
            return None;
        }
        Some(self.synced.partition_point(|(t, _)| *t <= pos) - 1)
    }
    pub fn next_change_after(&self, pos: f64) -> Option<f64> {
        self.synced.iter().map(|(t, _)| *t).find(|t| *t > pos).map(|t| t - pos)
    }
}

pub fn lyrics_key(t: &Track) -> String {
    format!("{}\u{1f}{}", t.title.to_lowercase(), t.artist.to_lowercase())
}

/// Parse format LRC: "[mm:ss.xx] teks" (boleh beberapa tag waktu per baris).
pub fn parse_lrc(src: &str) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    for line in src.lines() {
        let mut rest = line.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            if let Some(t) = parse_ts(tag) {
                times.push(t);
            }
            rest = &stripped[end + 1..];
        }
        let text = rest.trim().to_string();
        for t in times {
            out.push((t, text.clone()));
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn parse_ts(tag: &str) -> Option<f64> {
    let (m, s) = tag.split_once(':')?;
    let m: f64 = m.trim().parse().ok()?;
    let s: f64 = s.trim().replace(',', ".").parse().ok()?;
    Some(m * 60.0 + s)
}

/// Buang bagian dalam kurung: (Official Video), [Lyrics], 【MV】, dst.
fn strip_brackets(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' | '[' | '【' | '{' => depth += 1,
            ')' | ']' | '】' | '}' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Judul & artis yang dipakai untuk mencari lirik.
/// Video YouTube umumnya berjudul "Artis - Lagu (Official Music Video)" dengan nama
/// channel sebagai artis, jadi dibersihkan dulu.
pub fn lyrics_query(t: &Track) -> (String, String) {
    if t.source.is_empty() {
        return (t.title.clone(), t.artist.clone());
    }
    let mut title = strip_brackets(&t.title);
    let mut artist = t.artist.trim().to_string();
    for suffix in [" - Topic", "VEVO", " Official", " official"] {
        if let Some(a) = artist.strip_suffix(suffix) {
            artist = a.trim().to_string();
        }
    }
    for sep in [" - ", " – ", " — ", " | "] {
        if let Some((a, b)) = title.split_once(sep) {
            let (a, b) = (a.trim().to_string(), b.trim().to_string());
            if !a.is_empty() && !b.is_empty() {
                artist = a;
                title = b;
            }
            break;
        }
    }
    for noise in ["Official Music Video", "Official Video", "Official Audio", "Lyric Video", "Lyrics", "Video Klip", "Video Lirik"] {
        if let Some(i) = title.to_lowercase().find(&noise.to_lowercase()) {
            title.truncate(i);
        }
    }
    (title.trim().trim_end_matches(['-', '|', '·']).trim().to_string(), artist)
}

/// Ambil lirik dari LRCLIB. Hanya judul, artis, album, dan durasi yang dikirim.
pub fn fetch_lyrics(t: &Track) -> Option<Lyrics> {
    use crate::util::{http_get, urlencode};
    let base = "https://lrclib.net/api";
    let (title, artist) = lyrics_query(t);
    if title.is_empty() {
        return None;
    }
    let mut url = format!(
        "{base}/get?track_name={}&artist_name={}",
        urlencode(&title),
        urlencode(&artist)
    );
    if !t.album.is_empty() && t.source.is_empty() {
        url.push_str(&format!("&album_name={}", urlencode(&t.album)));
    }
    if t.duration > 0.0 {
        url.push_str(&format!("&duration={}", t.duration.round() as i64));
    }
    let parse_obj = |v: &serde_json::Value| -> Option<Lyrics> {
        let synced = v
            .get("syncedLyrics")
            .and_then(|s| s.as_str())
            .map(parse_lrc)
            .unwrap_or_default();
        let plain = v
            .get("plainLyrics")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        if synced.is_empty() && plain.is_none() {
            None
        } else {
            Some(Lyrics { synced, plain })
        }
    };
    if let Some(body) = http_get(&url) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(l) = parse_obj(&v) {
                return Some(l);
            }
        }
    }
    // Fallback: pencarian longgar.
    let url = format!(
        "{base}/search?track_name={}&artist_name={}",
        urlencode(&title),
        urlencode(&artist)
    );
    let body = http_get(&url)?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let arr = v.as_array()?;
    let best = arr
        .iter()
        .find(|o| o.get("syncedLyrics").and_then(|s| s.as_str()).is_some())
        .or_else(|| arr.first())?;
    parse_obj(best)
}

/// Decode & perkecil artwork jadi thumbnail 160px (hemat RAM).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn load_art(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = img.thumbnail(160, 160).to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lrc() {
        let l = parse_lrc("[ar:x]\n[00:01.50] Satu\n[00:05.00][01:00.00] Dua\nbukan\n[00:03,2]Tiga");
        assert_eq!(l.len(), 4);
        assert_eq!(l[0], (1.5, "Satu".to_string()));
        assert_eq!(l[1].1, "Tiga");
        assert_eq!(l[3].0, 60.0);
        let ly = Lyrics { synced: l, plain: None };
        assert_eq!(ly.current_index(0.5), None);
        assert_eq!(ly.current_index(4.0), Some(1));
        assert_eq!(ly.next_change_after(4.0), Some(1.0));
    }

    #[test]
    fn judul_youtube_dibersihkan() {
        let yt = |title: &str, artist: &str| {
            lyrics_query(&Track {
                title: title.into(),
                artist: artist.into(),
                source: "YouTube · Chrome".into(),
                ..Default::default()
            })
        };
        assert_eq!(
            yt("Tulus - Hati-Hati di Jalan (Official Music Video)", "TULUS"),
            ("Hati-Hati di Jalan".into(), "Tulus".into())
        );
        assert_eq!(yt("Lagu Untukmu [Lyrics]", "Nadin Amizah - Topic"), ("Lagu Untukmu".into(), "Nadin Amizah".into()));
        assert_eq!(yt("Bohemian Rhapsody", "QueenVEVO"), ("Bohemian Rhapsody".into(), "Queen".into()));
        assert_eq!(yt("Sal Priadi – Gala Bunga Matahari Official Video", "Sal Priadi"), ("Gala Bunga Matahari".into(), "Sal Priadi".into()));
        // pemutar native tidak diubah
        let native = Track { title: "Song (Live)".into(), artist: "A".into(), ..Default::default() };
        assert_eq!(lyrics_query(&native), ("Song (Live)".into(), "A".into()));
    }
}

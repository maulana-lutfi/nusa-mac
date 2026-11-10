//! Musik dari tab browser — bagian lintas-platform (bisa dites di Linux).
//!
//! Cara kerja di macOS (lihat `platform/macos.rs`):
//! 1. AppleScript mengambil daftar URL tab (satu Apple Event per window) di browser yang
//!    sedang terbuka, lalu hanya menyentuh tab situs musik (`MEDIA_SITES`).
//! 2. Di tab itu dijalankan `JS_STATE`, yang membaca Media Session API + elemen
//!    `<video>/<audio>`: judul, artis/channel, album, sampul, posisi, durasi, status putar.
//! 3. Kontrol (play/pause, next, prev, seek) juga dijalankan sebagai JavaScript.
//!
//! Butuh "Allow JavaScript from Apple Events" di browser. Tanpa itu, hanya judul tab yang
//! terbaca (`Candidate::state == None` → `Track::no_control`).

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::media::{MediaCmd, Track};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Browser {
    Chrome,
    Arc,
    Edge,
    Brave,
    Safari,
}

impl Browser {
    pub const ALL: [Browser; 5] = [Browser::Chrome, Browser::Arc, Browser::Edge, Browser::Brave, Browser::Safari];
    pub fn app(self) -> &'static str {
        match self {
            Browser::Chrome => "Google Chrome",
            Browser::Arc => "Arc",
            Browser::Edge => "Microsoft Edge",
            Browser::Brave => "Brave Browser",
            Browser::Safari => "Safari",
        }
    }
    pub fn bundle(self) -> &'static str {
        match self {
            Browser::Chrome => "com.google.Chrome",
            Browser::Arc => "company.thebrowser.Browser",
            Browser::Edge => "com.microsoft.edgemac",
            Browser::Brave => "com.brave.Browser",
            Browser::Safari => "com.apple.Safari",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Browser::Chrome => "Chrome",
            Browser::Arc => "Arc",
            Browser::Edge => "Edge",
            Browser::Brave => "Brave",
            Browser::Safari => "Safari",
        }
    }
}

/// Situs musik yang dipindai (pola URL, nama tampilan). Tab lain tidak pernah disentuh.
pub const MEDIA_SITES: [(&str, &str); 6] = [
    ("music.youtube.com", "YouTube Music"),
    ("youtube.com/watch", "YouTube"),
    ("youtube.com/live", "YouTube"),
    ("soundcloud.com", "SoundCloud"),
    ("open.spotify.com", "Spotify Web"),
    ("music.apple.com", "Apple Music Web"),
];

/// Ambil elemen media utama (pemutar utama YouTube diutamakan).
const JS_MEDIA: &str = "var m=document.querySelector('video.html5-main-video')||document.querySelector('video,audio');";

/// Membaca status halaman. Tanpa tanda kutip ganda & backslash supaya aman di string AppleScript.
pub const JS_STATE: &str = "(function(){var m=document.querySelector('video.html5-main-video')||document.querySelector('video,audio');var ms=navigator.mediaSession;var s=ms&&ms.metadata;if(!m&&!s){return 'NONE'}var art='';if(s&&s.artwork&&s.artwork.length){art=s.artwork[s.artwork.length-1].src}var p=m?!m.paused:(ms&&ms.playbackState=='playing');return JSON.stringify({p:p,c:m?m.currentTime:0,d:(m&&isFinite(m.duration))?m.duration:0,t:s?s.title:document.title,a:s?s.artist:'',al:s?s.album:'',art:art})})()";

pub fn js_command(c: MediaCmd) -> String {
    let m = JS_MEDIA;
    match c {
        MediaCmd::PlayPause => format!("(function(){{{m}if(m){{if(m.paused){{m.play()}}else{{m.pause()}}}}return 'ok'}})()"),
        MediaCmd::Pause => format!("(function(){{{m}if(m){{m.pause()}}return 'ok'}})()"),
        MediaCmd::Seek(s) => format!("(function(){{{m}if(m){{m.currentTime={:.2}}}return 'ok'}})()", s.max(0.0)),
        MediaCmd::Next => "(function(){var b=document.querySelector('.ytp-next-button,.next-button,[data-testid=control-button-skip-forward],[aria-label=Next],[title=Next]');if(b){b.click()}return 'ok'})()".into(),
        MediaCmd::Prev => format!("(function(){{var b=document.querySelector('.previous-button,[data-testid=control-button-skip-back],[aria-label=Previous],[title=Previous]');{m}if(b){{b.click()}}else if(m){{m.currentTime=0}}return 'ok'}})()"),
    }
}

/// Escape untuk literal string AppleScript.
pub trait AsQuote {
    fn as_quoted(&self) -> String;
}
impl AsQuote for str {
    fn as_quoted(&self) -> String {
        self.replace('\\', "\\\\").replace('"', "\\\"")
    }
}

/// Script yang menelusuri tab, lalu menjalankan `js` hanya di tab yang cocok dengan `filter`
/// (ekspresi AppleScript atas variabel `u` = URL tab).
/// Output per tab: URL <TAB> judul tab <TAB> hasil JS (atau "ERR" bila JS tidak diizinkan).
pub fn tabs_script(b: Browser, filter: &str, js: &str) -> String {
    let js = js.as_quoted();
    let (exec, title) = if b == Browser::Safari {
        (format!("do JavaScript \"{js}\" in t"), "name of t")
    } else {
        (format!("execute t javascript \"{js}\""), "title of t")
    };
    format!(
        r#"set TB to character id 9
set out to ""
tell application "{app}"
	repeat with w in windows
		try
			set urls to URL of every tab of w
			repeat with i from 1 to count of urls
				set u to item i of urls
				if u is not missing value then
					if {filter} then
						set t to tab i of w
						set r to "ERR"
						try
							set r to ({exec}) as text
						end try
						set out to out & u & TB & ({title}) & TB & r & linefeed
					end if
				end if
			end repeat
		end try
	end repeat
end tell
return out"#,
        app = b.app()
    )
}

pub fn media_filter() -> String {
    let parts: Vec<String> = MEDIA_SITES.iter().map(|(pat, _)| format!("u contains \"{pat}\"")).collect();
    format!("({})", parts.join(" or "))
}

#[derive(serde::Deserialize, Default, Debug, Clone, PartialEq)]
pub struct PageState {
    #[serde(default)]
    pub p: bool,
    #[serde(default)]
    pub c: f64,
    #[serde(default)]
    pub d: f64,
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub a: String,
    #[serde(default)]
    pub al: String,
    #[serde(default)]
    pub art: String,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub browser: Browser,
    pub url: String,
    pub tab_title: String,
    pub state: Option<PageState>,
}

impl Candidate {
    pub fn to_track(&self) -> Track {
        let source = format!("{} · {}", site_name(&self.url), self.browser.short());
        match &self.state {
            Some(s) => Track {
                id: self.url.clone(),
                title: if s.t.trim().is_empty() { clean_tab_title(&self.tab_title) } else { s.t.trim().to_string() },
                artist: s.a.trim().to_string(),
                album: s.al.trim().to_string(),
                duration: if s.d.is_finite() { s.d.max(0.0) } else { 0.0 },
                position: s.c.max(0.0),
                playing: s.p,
                source,
                no_control: false,
            },
            None => Track {
                id: self.url.clone(),
                title: clean_tab_title(&self.tab_title),
                source,
                no_control: true,
                ..Default::default()
            },
        }
    }
}

pub fn site_name(url: &str) -> &'static str {
    MEDIA_SITES
        .iter()
        .find(|(pat, _)| url.contains(pat))
        .map(|(_, n)| *n)
        .unwrap_or("Web")
}

/// "(3) Judul - YouTube" → "Judul".
pub fn clean_tab_title(t: &str) -> String {
    let mut s = t.trim();
    if s.starts_with('(') {
        if let Some(i) = s.find(") ") {
            if i > 1 && s[1..i].chars().all(|c| c.is_ascii_digit()) {
                s = &s[i + 2..];
            }
        }
    }
    for suffix in [" - YouTube Music", " - YouTube", " | SoundCloud", " | Spotify", " - Apple Music"] {
        if let Some(x) = s.strip_suffix(suffix) {
            return x.trim().to_string();
        }
    }
    s.to_string()
}

/// Baris-baris "URL<TAB>judul<TAB>hasil" dari `tabs_script`.
pub fn parse_poll_output(b: Browser, out: &str) -> Vec<Candidate> {
    out.lines()
        .filter_map(|line| {
            let mut it = line.splitn(3, '\t');
            let (url, title, res) = (it.next()?, it.next()?, it.next()?);
            let state = match res.trim() {
                "NONE" => return None, // tab musik tanpa media (mis. beranda YouTube)
                "ERR" | "" | "missing value" => None,
                json => Some(serde_json::from_str::<PageState>(json).ok()?),
            };
            Some(Candidate { browser: b, url: url.to_string(), tab_title: title.to_string(), state })
        })
        .collect()
}

/// Prioritas: sedang diputar → pernah diputar (posisi > 0) → ada data → judul saja.
pub fn pick_best(c: &[Candidate]) -> Option<&Candidate> {
    c.iter()
        .find(|c| c.state.as_ref().is_some_and(|s| s.p))
        .or_else(|| c.iter().find(|c| c.state.as_ref().is_some_and(|s| s.c > 0.5)))
        .or_else(|| c.iter().find(|c| c.state.is_some()))
        .or_else(|| c.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judul_tab() {
        assert_eq!(clean_tab_title("(3) Tulus - Hati-Hati di Jalan - YouTube"), "Tulus - Hati-Hati di Jalan");
        assert_eq!(clean_tab_title("Lagu - YouTube Music"), "Lagu");
        assert_eq!(clean_tab_title("(Live) Lagu - YouTube"), "(Live) Lagu");
    }

    #[test]
    fn parse_dan_pilih() {
        let out = "https://www.youtube.com/watch?v=a\tA - YouTube\t{\"p\":false,\"c\":12.5,\"d\":200,\"t\":\"Lagu A\",\"a\":\"Kanal\",\"al\":\"\",\"art\":\"https://i.ytimg.com/a.jpg\"}\n\
https://www.youtube.com/\tYouTube\tNONE\n\
https://music.youtube.com/watch?v=b\tB - YouTube Music\t{\"p\":true,\"c\":3,\"d\":180,\"t\":\"Lagu B\",\"a\":\"Artis\",\"al\":\"Album\",\"art\":\"\"}\n\
https://soundcloud.com/x\tX | SoundCloud\tERR\n";
        let c = parse_poll_output(Browser::Chrome, out);
        assert_eq!(c.len(), 3, "tab tanpa media (NONE) diabaikan");
        let best = pick_best(&c).unwrap();
        assert_eq!(best.url, "https://music.youtube.com/watch?v=b", "yang sedang diputar menang");
        let t = best.to_track();
        assert_eq!((t.title.as_str(), t.artist.as_str(), t.source.as_str()), ("Lagu B", "Artis", "YouTube Music · Chrome"));
        assert!(t.playing && !t.no_control);

        // tanpa izin JavaScript: hanya judul tab
        let only = parse_poll_output(Browser::Safari, "https://soundcloud.com/x\tX | SoundCloud\tERR\n");
        let t = pick_best(&only).unwrap().to_track();
        assert_eq!((t.title.as_str(), t.no_control, t.source.as_str()), ("X", true, "SoundCloud · Safari"));

        // pernah diputar lebih diutamakan dari yang belum
        let c2 = parse_poll_output(
            Browser::Brave,
            "u1\tt1\t{\"p\":false,\"c\":0}\nhttps://www.youtube.com/watch?v=z\tt2\t{\"p\":false,\"c\":40}\n",
        );
        assert_eq!(pick_best(&c2).unwrap().url, "https://www.youtube.com/watch?v=z");
    }

    #[test]
    fn script_aman() {
        // JavaScript tidak boleh mengandung kutip ganda / backslash (literal AppleScript)
        for js in [JS_STATE.to_string(), js_command(MediaCmd::PlayPause), js_command(MediaCmd::Next), js_command(MediaCmd::Prev), js_command(MediaCmd::Seek(61.5)), js_command(MediaCmd::Pause)] {
            assert!(!js.contains('"') && !js.contains('\\'), "{js}");
        }
        let s = tabs_script(Browser::Chrome, &media_filter(), JS_STATE);
        assert!(s.contains("tell application \"Google Chrome\""));
        assert!(s.contains("execute t javascript \"(function(){"));
        assert!(s.contains("u contains \"youtube.com/watch\" or"));
        let s = tabs_script(Browser::Safari, "u is \"x\"", "1+1");
        assert!(s.contains("do JavaScript \"1+1\" in t") && s.contains("name of t"));
        assert_eq!("a\"b\\c".as_quoted(), "a\\\"b\\\\c");
        assert!(js_command(MediaCmd::Seek(61.5)).contains("m.currentTime=61.50"));
    }
}

#[cfg(test)]
mod dump {
    use super::*;
    /// `cargo test dump_js -- --ignored --nocapture` → cetak semua JavaScript (untuk diuji di Node).
    #[test]
    #[ignore]
    fn dump_js() {
        println!("JS\tstate\t{JS_STATE}");
        for (n, c) in [("playpause", MediaCmd::PlayPause), ("pause", MediaCmd::Pause), ("next", MediaCmd::Next), ("prev", MediaCmd::Prev), ("seek", MediaCmd::Seek(61.5))] {
            println!("JS\t{n}\t{}", js_command(c));
        }
    }
}

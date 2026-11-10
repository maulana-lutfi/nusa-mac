//! Perhitungan waktu sholat lokal (tanpa internet).
//! Algoritma astronomi standar (gaya PrayTimes.org) dengan parameter Kemenag RI:
//! Subuh 20°, Isya 18°, Ashar madzhab Syafi'i (bayangan 1x), ihtiyat ±2–4 menit.
//! Diuji terhadap jadwal resmi Kemenag Jakarta (lihat test): selisih ≤ 1 menit.

use chrono::{Datelike, NaiveDate};

#[derive(Clone, Debug)]
pub struct City {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
}

pub const CITIES: &[City] = &[
    City { name: "Jakarta", lat: -6.2088, lon: 106.8456 },
    City { name: "Bogor", lat: -6.5971, lon: 106.8060 },
    City { name: "Depok", lat: -6.4025, lon: 106.7942 },
    City { name: "Tangerang", lat: -6.1783, lon: 106.6319 },
    City { name: "Bekasi", lat: -6.2383, lon: 106.9756 },
    City { name: "Bandung", lat: -6.9175, lon: 107.6191 },
    City { name: "Semarang", lat: -6.9667, lon: 110.4167 },
    City { name: "Yogyakarta", lat: -7.7956, lon: 110.3695 },
    City { name: "Solo", lat: -7.5755, lon: 110.8243 },
    City { name: "Surabaya", lat: -7.2575, lon: 112.7521 },
    City { name: "Malang", lat: -7.9666, lon: 112.6326 },
    City { name: "Denpasar", lat: -8.6705, lon: 115.2126 },
    City { name: "Mataram", lat: -8.5833, lon: 116.1167 },
    City { name: "Banda Aceh", lat: 5.5483, lon: 95.3238 },
    City { name: "Medan", lat: 3.5952, lon: 98.6722 },
    City { name: "Padang", lat: -0.9471, lon: 100.4172 },
    City { name: "Pekanbaru", lat: 0.5071, lon: 101.4478 },
    City { name: "Batam", lat: 1.0456, lon: 104.0305 },
    City { name: "Palembang", lat: -2.9761, lon: 104.7754 },
    City { name: "Bandar Lampung", lat: -5.3971, lon: 105.2668 },
    City { name: "Pontianak", lat: -0.0263, lon: 109.3425 },
    City { name: "Banjarmasin", lat: -3.3186, lon: 114.5944 },
    City { name: "Balikpapan", lat: -1.2379, lon: 116.8529 },
    City { name: "Samarinda", lat: -0.5022, lon: 117.1536 },
    City { name: "Makassar", lat: -5.1477, lon: 119.4327 },
    City { name: "Manado", lat: 1.4748, lon: 124.8421 },
    City { name: "Ambon", lat: -3.6954, lon: 128.1814 },
    City { name: "Jayapura", lat: -2.5337, lon: 140.7181 },
    City { name: "Kuala Lumpur", lat: 3.1390, lon: 101.6869 },
    City { name: "Singapura", lat: 1.3521, lon: 103.8198 },
];

pub const NAMES: [&str; 5] = ["Subuh", "Dzuhur", "Ashar", "Maghrib", "Isya"];

/// Waktu dalam jam desimal lokal (mis. 4.5 = 04:30).
#[derive(Clone, Debug, PartialEq)]
pub struct DayTimes {
    pub fajr: f64,
    pub sunrise: f64,
    pub dhuhr: f64,
    pub asr: f64,
    pub sunset: f64,
    pub maghrib: f64,
    pub isha: f64,
}

impl DayTimes {
    pub fn prayers(&self) -> [f64; 5] {
        [self.fajr, self.dhuhr, self.asr, self.maghrib, self.isha]
    }
}

fn dsin(d: f64) -> f64 { d.to_radians().sin() }
fn dcos(d: f64) -> f64 { d.to_radians().cos() }
fn dtan(d: f64) -> f64 { d.to_radians().tan() }
fn darcsin(x: f64) -> f64 { x.asin().to_degrees() }
fn darccos(x: f64) -> f64 { x.clamp(-1.0, 1.0).acos().to_degrees() }
fn darctan2(y: f64, x: f64) -> f64 { y.atan2(x).to_degrees() }
fn darccot(x: f64) -> f64 { (1.0 / x).atan().to_degrees() }
fn fix(a: f64, b: f64) -> f64 { let a = a - b * (a / b).floor(); if a < 0.0 { a + b } else { a } }

fn julian(y: i32, m: u32, d: u32) -> f64 {
    let (mut y, mut m) = (y as f64, m as f64);
    if m <= 2.0 {
        y -= 1.0;
        m += 12.0;
    }
    let a = (y / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y + 4716.0)).floor() + (30.6001 * (m + 1.0)).floor() + d as f64 + b - 1524.5
}

/// (deklinasi, equation of time)
fn sun_position(jd: f64) -> (f64, f64) {
    let d = jd - 2451545.0;
    let g = fix(357.529 + 0.98560028 * d, 360.0);
    let q = fix(280.459 + 0.98564736 * d, 360.0);
    let l = fix(q + 1.915 * dsin(g) + 0.020 * dsin(2.0 * g), 360.0);
    let e = 23.439 - 0.00000036 * d;
    let ra = darctan2(dcos(e) * dsin(l), dcos(l)) / 15.0;
    let eqt = q / 15.0 - fix(ra, 24.0);
    let decl = darcsin(dsin(e) * dsin(l));
    (decl, eqt)
}

pub fn compute(date: NaiveDate, lat: f64, lon: f64, tz_hours: f64) -> DayTimes {
    let jd = julian(date.year(), date.month(), date.day()) - lon / (15.0 * 24.0);
    let mid_day = |t: f64| fix(12.0 - sun_position(jd + t).1, 24.0);
    let sun_angle_time = |angle: f64, t: f64, ccw: bool| {
        let decl = sun_position(jd + t).0;
        let noon = mid_day(t);
        let tt = darccos((-dsin(angle) - dsin(decl) * dsin(lat)) / (dcos(decl) * dcos(lat))) / 15.0;
        noon + if ccw { -tt } else { tt }
    };
    let asr_time = |factor: f64, t: f64| {
        let decl = sun_position(jd + t).0;
        let angle = -darccot(factor + dtan((lat - decl).abs()));
        sun_angle_time(angle, t, false)
    };

    let fajr = sun_angle_time(20.0, 5.0 / 24.0, true);
    let sunrise = sun_angle_time(0.833, 6.0 / 24.0, true);
    let dhuhr = mid_day(12.0 / 24.0);
    let asr = asr_time(1.0, 13.0 / 24.0);
    let sunset = sun_angle_time(0.833, 18.0 / 24.0, false);
    let isha = sun_angle_time(18.0, 18.0 / 24.0, false);

    let adj = tz_hours - lon / 15.0;
    // Ihtiyat (menit) dikalibrasi terhadap jadwal resmi Kemenag RI.
    let m = |x: f64| x / 60.0;
    DayTimes {
        fajr: fajr + adj + m(2.4),
        sunrise: sunrise + adj - m(2.0),
        dhuhr: dhuhr + adj + m(3.6),
        asr: asr + adj + m(2.7),
        sunset: sunset + adj,
        maghrib: sunset + adj + m(3.2),
        isha: isha + adj + m(2.7),
    }
}

pub fn fmt_hhmm(h: f64) -> String {
    let total = (fix(h, 24.0) * 60.0).round() as i64 % (24 * 60);
    format!("{:02}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hm(s: &str) -> f64 {
        let (h, m) = s.split_once(':').unwrap();
        h.parse::<f64>().unwrap() + m.parse::<f64>().unwrap() / 60.0
    }

    #[test]
    fn cocok_dengan_jadwal_kemenag_jakarta() {
        // Data resmi Kemenag RI (via liputan6.com/jadwal-sholat/kota-jakarta), WIB.
        let cases = [
            ((2026, 11, 10), ["04:06", "11:40", "14:59", "17:51", "19:03"]),
            ((2026, 2, 10), ["04:39", "12:10", "15:26", "18:20", "19:31"]),
            ((2026, 8, 10), ["04:44", "12:02", "15:22", "17:58", "19:09"]),
            ((2026, 10, 1), ["04:23", "11:46", "14:51", "17:50", "18:59"]),
        ];
        let mut worst: f64 = 0.0;
        for ((y, m, d), exp) in cases {
            let t = compute(NaiveDate::from_ymd_opt(y, m, d).unwrap(), -6.2088, 106.8456, 7.0);
            for (i, (got, e)) in t.prayers().iter().zip(exp).enumerate() {
                let diff = (got - hm(e)) * 60.0;
                println!("{y}-{m:02}-{d:02} {:8} hitung {} resmi {} selisih {:+.1} mnt", NAMES[i], fmt_hhmm(*got), e, diff);
                worst = worst.max(diff.abs());
            }
        }
        assert!(worst <= 1.0, "selisih terbesar {worst:.1} menit");
    }
}

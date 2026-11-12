//! Kartbild över en adress, från OpenStreetMap. Adressen slås upp hos Nominatim och
//! kartan sätts ihop av rutor från OSM:s kartserver. Inget konto och ingen nyckel behövs,
//! men OSM kräver att upphovet anges i bilden — det sköter den som lägger in kartan.
//!
//! Det enda som skickas ut är adressen användaren själv skrev in.

use image::RgbaImage;

/// Utsnitt: namn och zoomnivå.
pub const LEVELS: [(&str, u32); 4] = [("Gata", 17), ("Kvarter", 16), ("Område", 14), ("Ort", 12)];
pub const ATTRIBUTION: &str = "© OpenStreetMap";
const TILE: u32 = 256;
pub const WIDTH: u32 = 768;
pub const HEIGHT: u32 = 576;

pub struct MapResult {
    pub image: RgbaImage,
    /// Platsens namn så som Nominatim skriver det.
    pub place: String,
}

fn agent() -> ureq::Agent {
    use ureq::tls::{TlsConfig, TlsProvider};
    ureq::Agent::config_builder()
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
        // OSM:s villkor kräver att programmet talar om vad det är.
        .user_agent(concat!("TRK-ForceShot/", env!("CARGO_PKG_VERSION"), " (+https://github.com/YamYam-ee/forceshot2)"))
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .build()
        .new_agent()
}

pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Longitud/latitud till rutkoordinater (med decimaler) på en given zoomnivå.
pub fn tile_xy(lat: f64, lon: f64, zoom: u32) -> (f64, f64) {
    let n = 2f64.powi(zoom as i32);
    let x = (lon + 180.0) / 360.0 * n;
    let r = lat.to_radians();
    let y = (1.0 - (r.tan() + 1.0 / r.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}

fn geocode(agent: &ureq::Agent, query: &str) -> Result<(f64, f64, String), String> {
    let url = format!("https://nominatim.openstreetmap.org/search?format=jsonv2&limit=1&accept-language=sv&q={}", encode(query));
    let mut resp = agent.get(&url).call().map_err(|e| format!("Kunde inte nå karttjänsten: {e}"))?;
    let json: serde_json::Value = resp.body_mut().read_json().map_err(|e| format!("Oväntat svar från karttjänsten: {e}"))?;
    let hit = json.as_array().and_then(|a| a.first()).ok_or("Hittade ingen plats med den adressen.")?;
    let num = |k: &str| hit.get(k).and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok());
    let (lat, lon) = (num("lat").ok_or("Svaret saknar koordinater.")?, num("lon").ok_or("Svaret saknar koordinater.")?);
    let name = hit.get("display_name").and_then(|v| v.as_str()).unwrap_or(query).to_string();
    Ok((lat, lon, name))
}

/// Slår upp adressen och sätter ihop en kartbild med platsen i mitten.
pub fn fetch(query: &str, zoom: u32) -> Result<MapResult, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Skriv en adress först.".into());
    }
    let agent = agent();
    let (lat, lon, place) = geocode(&agent, query)?;
    let (cx, cy) = tile_xy(lat, lon, zoom);
    // Bildens övre vänstra hörn i världens bildpunkter.
    let left = (cx * TILE as f64 - WIDTH as f64 / 2.0).floor() as i64;
    let top = (cy * TILE as f64 - HEIGHT as f64 / 2.0).floor() as i64;
    let max = 1i64 << zoom;
    let mut image = RgbaImage::from_pixel(WIDTH, HEIGHT, image::Rgba([0xE8, 0xE8, 0xE8, 255]));
    let (tx0, ty0) = (left.div_euclid(TILE as i64), top.div_euclid(TILE as i64));
    let (tx1, ty1) = ((left + WIDTH as i64 - 1).div_euclid(TILE as i64), (top + HEIGHT as i64 - 1).div_euclid(TILE as i64));
    for ty in ty0..=ty1 {
        if ty < 0 || ty >= max {
            continue;
        }
        for tx in tx0..=tx1 {
            let url = format!("https://tile.openstreetmap.org/{zoom}/{}/{ty}.png", tx.rem_euclid(max));
            let resp = agent.get(&url).call().map_err(|e| format!("Kunde inte hämta kartan: {e}"))?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut resp.into_body().into_with_config().limit(4 * 1024 * 1024).reader(), &mut bytes).map_err(|e| e.to_string())?;
            let tile = image::load_from_memory(&bytes).map_err(|e| format!("Kartrutan gick inte att läsa: {e}"))?.to_rgba8();
            image::imageops::replace(&mut image, &tile, tx * TILE as i64 - left, ty * TILE as i64 - top);
        }
    }
    Ok(MapResult { image, place })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_swedish_addresses() {
        assert_eq!(encode("Storgatan 1, Malmö"), "Storgatan%201%2C%20Malm%C3%B6");
    }

    #[test]
    fn tile_coordinates_match_known_values() {
        // Nollmeridianen vid ekvatorn ligger mitt i världen.
        let (x, y) = tile_xy(0.0, 0.0, 1);
        assert!((x - 1.0).abs() < 1e-9 && (y - 1.0).abs() < 1e-9);
        // Malmö (55,605 N, 13,003 O) på nivå 12 ligger i ruta 2195/1283.
        let (x, y) = tile_xy(55.605, 13.003, 12);
        assert_eq!((x.floor() as i64, y.floor() as i64), (2195, 1283));
    }
}

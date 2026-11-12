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

/// Kartans utsnitt: mittpunkten i världens bildpunkter på zoomnivån.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: f64,
    pub y: f64,
    pub zoom: u32,
}

pub const MIN_ZOOM: u32 = 3;
pub const MAX_ZOOM: u32 = 19;

impl View {
    pub fn at(lat: f64, lon: f64, zoom: u32) -> Self {
        let (x, y) = tile_xy(lat, lon, zoom);
        Self { x: x * TILE as f64, y: y * TILE as f64, zoom }
    }

    /// Flyttar utsnittet så många bildpunkter. I höjdled tar världen slut.
    pub fn moved(self, dx: f64, dy: f64) -> Self {
        let world = (TILE as f64) * 2f64.powi(self.zoom as i32);
        Self { x: (self.x + dx).rem_euclid(world), y: (self.y + dy).clamp(0.0, world), zoom: self.zoom }
    }

    /// Samma mittpunkt på en annan zoomnivå.
    pub fn with_zoom(self, zoom: u32) -> Self {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let k = 2f64.powi(zoom as i32 - self.zoom as i32);
        Self { x: self.x * k, y: self.y * k, zoom }
    }
}

/// Hämtar kartrutor och minns dem, så att en ruta bara hämtas en gång medan man
/// flyttar runt utsnittet.
pub struct Tiles {
    agent: ureq::Agent,
    cache: std::collections::HashMap<(u32, i64, i64), RgbaImage>,
}

impl Default for Tiles {
    fn default() -> Self {
        Self { agent: agent(), cache: Default::default() }
    }
}

impl Tiles {
    /// Slår upp en adress: latitud, longitud och platsens namn.
    pub fn locate(&self, query: &str) -> Result<(f64, f64, String), String> {
        let query = query.trim();
        if query.is_empty() {
            return Err("Skriv en adress först.".into());
        }
        geocode(&self.agent, query)
    }

    fn tile(&mut self, zoom: u32, tx: i64, ty: i64) -> Result<&RgbaImage, String> {
        use std::collections::hash_map::Entry;
        if self.cache.len() > 400 {
            self.cache.clear();
        }
        match self.cache.entry((zoom, tx, ty)) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let url = format!("https://tile.openstreetmap.org/{zoom}/{tx}/{ty}.png");
                let resp = self.agent.get(&url).call().map_err(|e| format!("Kunde inte hämta kartan: {e}"))?;
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut resp.into_body().into_with_config().limit(4 * 1024 * 1024).reader(), &mut bytes).map_err(|e| e.to_string())?;
                let tile = image::load_from_memory(&bytes).map_err(|e| format!("Kartrutan gick inte att läsa: {e}"))?.to_rgba8();
                Ok(e.insert(tile))
            }
        }
    }

    /// Sätter ihop kartbilden för ett utsnitt.
    pub fn render(&mut self, view: View) -> Result<RgbaImage, String> {
        let zoom = view.zoom;
        // Bildens övre vänstra hörn i världens bildpunkter.
        let left = (view.x - WIDTH as f64 / 2.0).floor() as i64;
        let top = (view.y - HEIGHT as f64 / 2.0).floor() as i64;
        let max = 1i64 << zoom;
        let mut image = RgbaImage::from_pixel(WIDTH, HEIGHT, image::Rgba([0xE8, 0xE8, 0xE8, 255]));
        let (tx0, ty0) = (left.div_euclid(TILE as i64), top.div_euclid(TILE as i64));
        let (tx1, ty1) = ((left + WIDTH as i64 - 1).div_euclid(TILE as i64), (top + HEIGHT as i64 - 1).div_euclid(TILE as i64));
        for ty in ty0..=ty1 {
            if ty < 0 || ty >= max {
                continue;
            }
            for tx in tx0..=tx1 {
                let tile = self.tile(zoom, tx.rem_euclid(max), ty)?.clone();
                image::imageops::replace(&mut image, &tile, tx * TILE as i64 - left, ty * TILE as i64 - top);
            }
        }
        Ok(image)
    }
}

/// Slår upp adressen och sätter ihop en kartbild med platsen i mitten.
pub fn fetch(query: &str, zoom: u32) -> Result<MapResult, String> {
    let mut tiles = Tiles::default();
    let (lat, lon, place) = tiles.locate(query)?;
    Ok(MapResult { image: tiles.render(View::at(lat, lon, zoom))?, place })
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

    #[test]
    fn view_keeps_its_centre_across_zoom_levels() {
        let v = View::at(55.605, 13.003, 16);
        let back = v.with_zoom(12).with_zoom(16);
        assert!((back.x - v.x).abs() < 1e-6 && (back.y - v.y).abs() < 1e-6);
        // Ruta 2195/1283 på nivå 12, alltså bildpunkt 2195·256 … 2196·256.
        let wide = v.with_zoom(12);
        assert_eq!(((wide.x / 256.0).floor() as i64, (wide.y / 256.0).floor() as i64), (2195, 1283));
        assert_eq!(v.with_zoom(40).zoom, MAX_ZOOM);
    }

    #[test]
    fn panning_wraps_sideways_but_not_upwards() {
        let v = View { x: 10.0, y: 10.0, zoom: 3 };
        let world = 256.0 * 8.0;
        let m = v.moved(-30.0, -30.0);
        assert!((m.x - (world - 20.0)).abs() < 1e-9);
        assert_eq!(m.y, 0.0);
    }
}

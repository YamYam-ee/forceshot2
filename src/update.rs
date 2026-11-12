//! Uppdatering från ett publikt GitHub-repo. Senaste utgåvan jämförs med den här
//! versionen; finns en nyare med en Setup-fil bifogad kan den hämtas, kontrolleras mot
//! utgåvans SHA-256 och köras. Inget annat än repots namn skickas till GitHub.

use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub file_name: String,
    pub url: String,
    pub size: u64,
    pub sha256: Option<String>,
}

/// Godtar både `ägare/repo` och hela adressen.
pub fn parse_repo(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/').trim_end_matches(".git");
    let s = s.strip_prefix("https://github.com/").or_else(|| s.strip_prefix("http://github.com/")).or_else(|| s.strip_prefix("github.com/")).unwrap_or(s);
    let mut parts = s.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    (ok(owner) && ok(repo) && parts.next().is_none()).then(|| format!("{owner}/{repo}"))
}

fn numbers(v: &str) -> Vec<u32> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect()
}

/// Är `candidate` en nyare version än `current`?
pub fn newer(candidate: &str, current: &str) -> bool {
    let (a, b) = (numbers(candidate), numbers(current));
    if a.is_empty() {
        return false;
    }
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// Plockar ut utgåvan ur GitHubs svar. `None` om den inte är nyare eller saknar Setup-fil.
pub fn parse_release(json: &serde_json::Value, current: &str) -> Option<Release> {
    let tag = json.get("tag_name")?.as_str()?;
    if !newer(tag, current) {
        return None;
    }
    let asset = json.get("assets")?.as_array()?.iter().find(|a| {
        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase();
        name.contains("setup") && name.ends_with(".exe")
    })?;
    let digest = asset.get("digest").and_then(|d| d.as_str()).and_then(|d| d.strip_prefix("sha256:")).map(|d| d.to_lowercase());
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        notes: json.get("body").and_then(|b| b.as_str()).unwrap_or("").trim().to_string(),
        file_name: asset.get("name")?.as_str()?.to_string(),
        url: asset.get("browser_download_url")?.as_str()?.to_string(),
        size: asset.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
        sha256: digest,
    })
}

fn agent() -> ureq::Agent {
    use ureq::tls::{TlsConfig, TlsProvider};
    ureq::Agent::config_builder()
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
        .user_agent("ForceShot-updater")
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build()
        .new_agent()
}

/// Frågar GitHub efter senaste utgåvan. `Ok(None)` betyder att den här versionen är den senaste.
pub fn check(repo: &str) -> Result<Option<Release>, String> {
    check_against(repo, crate::install::VERSION)
}

/// Som `check`, men jämför mot en angiven version.
pub fn check_against(repo: &str, current: &str) -> Result<Option<Release>, String> {
    let repo = parse_repo(repo).ok_or("Repot ska anges som ägare/repo.")?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let mut resp = agent().get(&url).header("Accept", "application/vnd.github+json").call().map_err(|e| format!("Kunde inte nå GitHub: {e}"))?;
    let json: serde_json::Value = resp.body_mut().read_json().map_err(|e| format!("Oväntat svar från GitHub: {e}"))?;
    Ok(parse_release(&json, current))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hämtar Setup-filen till en tillfällig mapp. Stämmer inte kontrollsumman kastas filen.
pub fn download(release: &Release, progress: &AtomicU64) -> Result<PathBuf, String> {
    use sha2::Digest;
    // Bara GitHubs egna adresser godtas, oavsett vad svaret påstår.
    if !release.url.starts_with("https://github.com/") {
        return Err("Utgåvans fil ligger inte på GitHub.".into());
    }
    let dir = std::env::temp_dir().join("ForceShot2-uppdatering");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = release.file_name.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
    let path = dir.join(safe);
    let resp = agent().get(&release.url).call().map_err(|e| format!("Hämtningen misslyckades: {e}"))?;
    let mut reader = resp.into_body().into_with_config().limit(500 * 1024 * 1024).reader();
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Hämtningen avbröts: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        progress.fetch_add(n as u64, Ordering::Relaxed);
    }
    drop(file);
    if let Some(want) = &release.sha256 {
        let got = hex(&hasher.finalize());
        if &got != want {
            let _ = std::fs::remove_file(&path);
            return Err("Den hämtade filen stämmer inte med utgåvans kontrollsumma och har kastats.".into());
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("v2.1.0", "2.0.0"));
        assert!(newer("2.0.1", "2.0.0"));
        assert!(newer("2.10.0", "2.9.9"));
        assert!(!newer("2.0.0", "2.0.0"));
        assert!(!newer("1.16.1", "2.0.0"));
        assert!(!newer("skräp", "2.0.0"));
        assert!(newer("3", "2.9.9"));
    }

    #[test]
    fn parses_repo_forms() {
        assert_eq!(parse_repo("agare/repo").as_deref(), Some("agare/repo"));
        assert_eq!(parse_repo(" https://github.com/agare/repo/ ").as_deref(), Some("agare/repo"));
        assert_eq!(parse_repo("https://github.com/agare/repo.git").as_deref(), Some("agare/repo"));
        assert_eq!(parse_repo("bara-ett-ord"), None);
        assert_eq!(parse_repo("a/b/c"), None);
        assert_eq!(parse_repo("a/b?x=1"), None);
    }

    #[test]
    fn picks_the_setup_asset_of_a_newer_release() {
        let json = serde_json::json!({
            "tag_name": "v2.1.0",
            "body": "Nytt: fiberskiss",
            "assets": [
                { "name": "kallkod.zip", "browser_download_url": "https://github.com/a/b/x.zip", "size": 5 },
                { "name": "TRK ForceShot Setup 2.1.0.exe", "browser_download_url": "https://github.com/a/b/setup.exe", "size": 123, "digest": "sha256:ABCDEF" }
            ]
        });
        let r = parse_release(&json, "2.0.0").expect("ska hitta utgåvan");
        assert_eq!(r.version, "2.1.0");
        assert_eq!(r.file_name, "TRK ForceShot Setup 2.1.0.exe");
        assert_eq!(r.size, 123);
        assert_eq!(r.sha256.as_deref(), Some("abcdef"));
        assert_eq!(r.notes, "Nytt: fiberskiss");
        assert!(parse_release(&json, "2.1.0").is_none());
        let utan = serde_json::json!({ "tag_name": "v9.0.0", "assets": [] });
        assert!(parse_release(&utan, "2.0.0").is_none());
    }
}

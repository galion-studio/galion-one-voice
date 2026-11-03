//! V2-S3 edge policy for the loopback phone surface.
//!
//! Pure decisions live here so the HTTP thread only applies them:
//! exact CORS, foreign-Origin rejection, loopback bind, a short-lived
//! session token, and Art. 50 disclosure strings. No secrets.

use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Spoken and visible Art. 50(1) line, Croatian. Keep in sync with the demo page.
pub const AI_DISCLOSURE_HR: &str =
    "Razgovarate s umjetnom inteligencijom. Galion je AI asistent, a ne osoba.";
/// Spoken and visible Art. 50(1) line, English. Keep in sync with the demo page.
pub const AI_DISCLOSURE_EN: &str =
    "You are talking to an AI. Galion is an AI assistant, not a person.";

/// How many live session tokens the process keeps. Oldest is wiped past this.
const SESSION_CAP: usize = 32;

/// What to do with one request's `Origin` header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginCheck {
    /// Proceed. `acao` is the `Access-Control-Allow-Origin` value, if any.
    Allow { acao: Option<String> },
    /// Foreign Origin, or a sensitive route with no Origin while an allow-list is set.
    /// Answer 403 and send no `Access-Control-Allow-Origin`.
    Reject,
}

/// Edge switches. The demo binary reads the environment; unit tests pin [`EdgeCfg::v1_demo`].
#[derive(Clone, Debug)]
pub struct EdgeCfg {
    /// Raw `GALION_CORS_ORIGINS`. Empty means the V1 wildcard.
    pub cors_origins: String,
    /// `GALION_REQUIRE_SESSION`.
    pub require_session: bool,
    /// `GALION_ALLOW_PUBLIC_BIND`. Still not enough on its own.
    pub allow_public_bind: bool,
    /// Art. 50 first-turn disclosure. On unless `GALION_AI_DISCLOSURE=0`.
    pub disclosure: bool,
    pub session_ttl: Duration,
}

impl EdgeCfg {
    /// Local demo: wildcard CORS, no session requirement, loopback only, disclosure on.
    pub fn v1_demo() -> Self {
        Self {
            cors_origins: String::new(),
            require_session: false,
            allow_public_bind: false,
            disclosure: true,
            session_ttl: Duration::from_secs(120),
        }
    }

    pub fn from_env() -> Self {
        let cors = std::env::var("GALION_CORS_ORIGINS").unwrap_or_default();
        let require = std::env::var("GALION_REQUIRE_SESSION").unwrap_or_default();
        let allow_public = std::env::var("GALION_ALLOW_PUBLIC_BIND").unwrap_or_default();
        let disclosure = std::env::var("GALION_AI_DISCLOSURE").ok();
        let ttl = std::env::var("GALION_SESSION_TTL_SECS").ok();
        edge_cfg(&cors, &require, &allow_public, disclosure.as_deref(), ttl.as_deref())
    }

    /// Wildcard is only the unset list. A literal `*` in the variable is not it.
    pub fn wildcard(&self) -> bool {
        self.cors_origins.trim().is_empty()
    }

    /// Exact allow-list, or the explicit session flag. Staging sets the list.
    pub fn session_required(&self) -> bool {
        self.require_session || cors_is_exact(&self.cors_origins)
    }
}

/// Build a config from already-read strings (tests call this; [`EdgeCfg::from_env`] too).
pub fn edge_cfg(
    cors: &str,
    require_session: &str,
    allow_public: &str,
    disclosure: Option<&str>,
    ttl_secs: Option<&str>,
) -> EdgeCfg {
    EdgeCfg {
        cors_origins: cors.to_string(),
        require_session: flag_on(require_session),
        allow_public_bind: flag_on(allow_public),
        disclosure: match disclosure {
            None => true,
            Some(v) => !matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"),
        },
        session_ttl: Duration::from_secs(parse_ttl(ttl_secs)),
    }
}

fn flag_on(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

fn parse_ttl(raw: Option<&str>) -> u64 {
    let Some(raw) = raw else { return 120 };
    match raw.trim().parse::<u64>() {
        Ok(n) => n.clamp(15, 900),
        Err(_) => 120,
    }
}

fn origin_token_ok(s: &str) -> bool {
    !s.is_empty()
        && s != "*"
        && s.len() <= 256
        && s.bytes().all(|b| b.is_ascii_graphic())
        && !s.bytes().any(|b| b == b',' || b == b'\\' )
}

/// Exact origins from a comma-separated list. `*` and blanks are dropped, not wildcards.
pub fn exact_origins(configured: &str) -> Vec<&str> {
    configured.split(',').map(str::trim).filter(|a| origin_token_ok(a)).collect()
}

pub fn cors_is_exact(configured: &str) -> bool {
    !exact_origins(configured).is_empty()
}

/// `GALION_CORS_ORIGINS` empty → `*` (V1, local demo only).
/// Any other value is an exact allow-list. A configured `*` matches nothing.
/// Staging must set the real `https://` origin. A foreign Origin yields `None`
/// (no `Access-Control-Allow-Origin`); the server answers 403.
pub fn cors_allow_origin(origin: Option<&str>, configured: &str) -> Option<String> {
    match check_origin(origin, configured, false) {
        OriginCheck::Allow { acao } => acao,
        OriginCheck::Reject => None,
    }
}

pub fn check_origin(origin: Option<&str>, configured: &str, sensitive: bool) -> OriginCheck {
    if configured.trim().is_empty() {
        return OriginCheck::Allow { acao: Some("*".to_string()) };
    }
    let origin = origin.unwrap_or("").trim();
    if origin.is_empty() {
        if sensitive {
            return OriginCheck::Reject;
        }
        return OriginCheck::Allow { acao: None };
    }
    if !origin_token_ok(origin) {
        return OriginCheck::Reject;
    }
    if exact_origins(configured).iter().any(|a| *a == origin) {
        OriginCheck::Allow { acao: Some(origin.to_string()) }
    } else {
        OriginCheck::Reject
    }
}

/// Loopback is the only bind that needs no extra switches.
/// `0.0.0.0` / `::` are refused unless the public-bind flag is on **and**
/// auth is on: exact `GALION_CORS_ORIGINS` plus `GALION_REQUIRE_SESSION`.
pub fn bind_decision(
    ip: IpAddr,
    allow_public: bool,
    cors: &str,
    require_session_flag: bool,
) -> Result<(), &'static str> {
    if ip.is_loopback() {
        return Ok(());
    }
    if !allow_public {
        return Err("web surface is localhost-only; refusing non-loopback bind (use an SSH tunnel)");
    }
    if !(cors_is_exact(cors) && require_session_flag) {
        return Err(
            "refusing public bind: GALION_ALLOW_PUBLIC_BIND requires GALION_REQUIRE_SESSION=1 and an exact GALION_CORS_ORIGINS list",
        );
    }
    Ok(())
}

pub fn route_is_sensitive(method: &str, path: &str) -> bool {
    let method = if method == "HEAD" { "GET" } else { method };
    match (method, path) {
        ("POST", "/turn" | "/mic" | "/session" | "/v2/say") => true,
        ("GET", p) if p.starts_with("/reply/") => true,
        ("OPTIONS", "/turn" | "/mic" | "/session" | "/v2/say") => true,
        ("OPTIONS", p) if p.starts_with("/reply/") => true,
        _ => false,
    }
}

/// Audio and mic posts, and reply fetches. `POST /session` mints a token, so it
/// does not require one. `OPTIONS` is a preflight and does not require one.
pub fn route_needs_session(method: &str, path: &str) -> bool {
    match (method, path) {
        ("POST", "/turn" | "/mic" | "/v2/say") => true,
        ("GET" | "HEAD", p) if p.starts_with("/reply/") => true,
        _ => false,
    }
}

/// Query keys that must never carry the session. The token belongs in the
/// first message body (or the `X-Galion-Session` header on a binary turn).
pub fn query_carries_token(query: &str) -> bool {
    query.split('&').any(|kv| {
        let key = kv.split_once('=').map(|(k, _)| k).unwrap_or(kv).trim();
        matches!(
            key.to_ascii_lowercase().as_str(),
            "token" | "session" | "access_token" | "galion_session"
        )
    })
}

pub fn parse_json_string_field(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let i = body.find(&needle)? + needle.len();
    let rest = body[i..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let v = &rest[..end];
    if v.is_empty() || v.len() > 128 {
        return None;
    }
    if v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        Some(v.to_string())
    } else {
        None
    }
}

/// Header first, then a JSON `"session"` field. Never reads the query string.
pub fn presented_session(header: Option<&str>, content_type: Option<&str>, body: &[u8]) -> Option<String> {
    if let Some(h) = header {
        let t = h.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let jsonish = content_type.unwrap_or("").to_ascii_lowercase().contains("json") || body.first() == Some(&b'{');
    if !jsonish {
        return None;
    }
    let s = std::str::from_utf8(body).ok()?;
    parse_json_string_field(s, "session")
}

/// Remove the demo disclosure block when `GALION_AI_DISCLOSURE=0`.
pub fn strip_ai_disclosure(page: &str) -> String {
    const START: &str = "<!--galion-ai-disclosure-->";
    const END: &str = "<!--/galion-ai-disclosure-->";
    match (page.find(START), page.find(END)) {
        (Some(i), Some(j)) if j >= i => {
            let mut s = String::with_capacity(page.len());
            s.push_str(&page[..i]);
            s.push_str(&page[j + END.len()..]);
            s
        }
        _ => page.to_string(),
    }
}

/// Overwrite then `black_box` so the wipe is not deleted as a dead store.
pub fn zeroise_bytes(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *b = 0;
    }
    std::hint::black_box(buf);
}

pub fn wipe_string(s: &mut String) {
    let mut v = std::mem::take(s).into_bytes();
    zeroise_bytes(&mut v);
}

fn tokens_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn mint_token() -> String {
    let mut b = [0u8; 32];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if std::io::Read::read_exact(&mut f, &mut b).is_ok() {
            return b.iter().map(|x| format!("{x:02x}")).collect();
        }
    }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{t:032x}{n:032x}")
}

struct Slot {
    token: String,
    exp: Instant,
}

/// RAM-only session tokens. Dropped slots are wiped.
pub struct SessionStore {
    slots: Mutex<std::collections::VecDeque<Slot>>,
    ttl: Duration,
}

impl SessionStore {
    pub fn new(ttl: Duration) -> Self {
        Self { slots: Mutex::new(std::collections::VecDeque::new()), ttl }
    }

    pub fn mint(&self) -> String {
        let token = mint_token();
        let exp = Instant::now() + self.ttl;
        if let Ok(mut g) = self.slots.lock() {
            purge(&mut g);
            while g.len() >= SESSION_CAP {
                if let Some(mut old) = g.pop_front() {
                    wipe_string(&mut old.token);
                }
            }
            g.push_back(Slot { token: token.clone(), exp });
        }
        token
    }

    pub fn check(&self, token: &str) -> bool {
        if token.len() != 64 {
            return false;
        }
        let Ok(mut g) = self.slots.lock() else { return false };
        purge(&mut g);
        g.iter().any(|s| s.exp > Instant::now() && tokens_eq(&s.token, token))
    }
}

fn purge(g: &mut std::collections::VecDeque<Slot>) {
    let now = Instant::now();
    while g.front().is_some_and(|s| s.exp <= now) {
        if let Some(mut old) = g.pop_front() {
            wipe_string(&mut old.token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn cors_star_only_when_unset_and_exact_otherwise() {
        assert_eq!(cors_allow_origin(None, ""), Some("*".into()));
        assert_eq!(cors_allow_origin(Some("https://a.ngrok-free.dev"), ""), Some("*".into()));
        assert_eq!(cors_allow_origin(Some("https://a.ngrok-free.dev"), "*"), None, "a configured star is not the V1 wildcard");
        assert_eq!(
            cors_allow_origin(Some("https://voice.example"), "https://voice.example, http://127.0.0.1:7878"),
            Some("https://voice.example".into())
        );
        assert_eq!(cors_allow_origin(Some("https://evil.example"), "https://voice.example"), None);
        assert_eq!(cors_allow_origin(Some("https://voice.example.evil"), "https://voice.example"), None);
        assert_eq!(cors_allow_origin(Some("https://voice.example"), "https://voice.example.evil"), None);
        assert_eq!(cors_allow_origin(None, "https://voice.example"), None);
        assert!(matches!(
            check_origin(Some("https://evil.example"), "https://voice.example", true),
            OriginCheck::Reject
        ));
        assert!(matches!(
            check_origin(None, "https://voice.example", true),
            OriginCheck::Reject
        ));
        assert!(matches!(
            check_origin(None, "https://voice.example", false),
            OriginCheck::Allow { acao: None }
        ));
    }

    #[test]
    fn public_bind_needs_flag_and_auth() {
        let any: IpAddr = "0.0.0.0".parse().unwrap();
        let lo: IpAddr = "127.0.0.1".parse().unwrap();
        let v6: IpAddr = "::".parse().unwrap();
        let v6lo: IpAddr = "::1".parse().unwrap();
        assert!(bind_decision(lo, false, "", false).is_ok());
        assert!(bind_decision(v6lo, false, "", false).is_ok());
        assert!(bind_decision(any, false, "https://voice.example", true).unwrap_err().contains("localhost-only"));
        assert!(bind_decision(v6, false, "", false).unwrap_err().contains("localhost-only"));
        assert!(bind_decision(any, true, "", true).unwrap_err().contains("GALION_REQUIRE_SESSION"));
        assert!(bind_decision(any, true, "*", true).unwrap_err().contains("exact"));
        assert!(bind_decision(any, true, "https://voice.example", false).is_err());
        assert!(bind_decision(any, true, "https://voice.example, http://127.0.0.1:7878", true).is_ok());
    }

    #[test]
    fn edge_flags_parse_without_reading_the_process_env() {
        let v1 = edge_cfg("", "", "", None, None);
        assert!(v1.wildcard() && v1.disclosure && !v1.session_required() && !v1.allow_public_bind);
        assert_eq!(v1.session_ttl, Duration::from_secs(120));
        let staged = edge_cfg("https://voice.example", "1", "0", Some("0"), Some("30"));
        assert!(!staged.wildcard() && staged.session_required() && !staged.disclosure);
        assert!(!staged.allow_public_bind);
        assert_eq!(staged.session_ttl, Duration::from_secs(30));
        let clamped = edge_cfg("https://voice.example", "yes", "on", Some("off"), Some("1"));
        assert_eq!(clamped.session_ttl, Duration::from_secs(15));
        assert!(clamped.require_session && clamped.allow_public_bind);
    }

    #[test]
    fn token_query_is_rejected_and_json_body_is_read() {
        assert!(query_carries_token("token=abc"));
        assert!(query_carries_token("rate=16000&session=abc"));
        assert!(!query_carries_token("rate=16000&channels=1"));
        assert!(!query_carries_token(""));
        let body = br#"{"rms":0.05,"session":"abc123"}"#;
        assert_eq!(presented_session(None, Some("application/json"), body).as_deref(), Some("abc123"));
        assert_eq!(presented_session(Some("from-header"), Some("application/json"), body).as_deref(), Some("from-header"));
        assert_eq!(presented_session(None, Some("audio/wav"), b"RIFF"), None);
    }

    #[test]
    fn pcm_bytes_and_strings_are_zeroised() {
        let mut b = b"RIFFPCM_SECRET".to_vec();
        zeroise_bytes(&mut b);
        assert!(b.iter().all(|&x| x == 0));
        let mut t = "SECRET_TRANSCRIPT".to_string();
        wipe_string(&mut t);
        assert!(t.is_empty());
    }

    #[test]
    fn disclosure_block_strips_on_the_flag() {
        let page = "before<!--galion-ai-disclosure--><p id=\"ai-disclosure\">secret</p><!--/galion-ai-disclosure-->after";
        let stripped = strip_ai_disclosure(page);
        assert_eq!(stripped, "beforeafter");
        assert!(!stripped.contains("ai-disclosure"));
        assert!(AI_DISCLOSURE_HR.contains("umjetnom inteligencijom"));
        assert!(AI_DISCLOSURE_EN.contains("AI assistant"));
    }

    #[test]
    fn session_token_is_short_lived_and_not_a_url() {
        let store = SessionStore::new(Duration::from_millis(40));
        let token = store.mint();
        assert_eq!(token.len(), 64);
        assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(!token.contains('?') && !token.contains('&') && !token.contains('/'));
        assert!(store.check(&token));
        assert!(!store.check("nope"));
        std::thread::sleep(Duration::from_millis(70));
        assert!(!store.check(&token), "expired token is rejected");
    }

    #[test]
    fn session_cap_wipes_the_oldest() {
        let store = SessionStore::new(Duration::from_secs(60));
        let mut first = String::new();
        for i in 0..(SESSION_CAP + 2) {
            let t = store.mint();
            if i == 0 {
                first = t;
            }
        }
        assert!(!store.check(&first), "oldest token was dropped");
    }
}

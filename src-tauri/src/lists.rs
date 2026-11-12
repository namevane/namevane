use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ureq::ResponseExt;
use uuid::Uuid;

use crate::pins;

const VERSION: u32 = 1;
const MAX_BYTES: usize = 512 * 1024;
const MAX_NAMES: usize = 2000;
const MAX_LISTS: usize = 30;
const INTERVALS: [u32; 5] = [0, 15, 60, 360, 1440];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ListsFile {
    version: u32,
    check_minutes: u32,
    last_check: u64,
    lists: Vec<Source>,
}

impl ListsFile {
    fn empty() -> Self {
        Self {
            version: VERSION,
            check_minutes: 0,
            last_check: 0,
            lists: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Source {
    id: String,
    title: String,
    url: String,
    enabled: bool,
    version: String,
    etag: String,
    checked_at: u64,
    error: Option<String>,
    entries: Vec<StoredEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredEntry {
    name: String,
    addresses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedList {
    version: String,
    title: String,
    entries: Vec<StoredEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceList {
    pub lists: Vec<SourceView>,
    pub check_minutes: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub id: String,
    pub title: String,
    pub url: String,
    pub enabled: bool,
    pub version: String,
    pub names: usize,
    pub checked_at: u64,
    pub error: Option<String>,
    pub entries: Vec<EntryView>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub name: String,
    pub addresses: Vec<String>,
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckJob {
    pub id: String,
    pub url: String,
    pub etag: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    Changed,
    Unchanged,
    Failed,
}

pub enum Download {
    NotModified,
    Updated { text: String, etag: String },
    Failed(String),
}

pub struct ListStore {
    path: PathBuf,
    file: ListsFile,
    error: Option<String>,
}

impl ListStore {
    pub fn open(path: PathBuf) -> Self {
        match load_file(&path) {
            Ok(mut file) => {
                if !INTERVALS.contains(&file.check_minutes) {
                    file.check_minutes = 0;
                }
                let before = file.lists.len();
                for source in &mut file.lists {
                    source.entries.retain(|entry| !entry.addresses.is_empty());
                }
                file.lists.retain(|source| !source.entries.is_empty());
                let store = Self {
                    path,
                    file,
                    error: None,
                };
                if store.file.lists.len() != before {
                    let _ = store.write();
                }
                store
            }
            Err(error) => Self {
                path,
                file: ListsFile::empty(),
                error: Some(error),
            },
        }
    }

    pub fn check_due(&self, now: u64) -> bool {
        let minutes = self.file.check_minutes;
        minutes > 0 && now.saturating_sub(self.file.last_check) >= u64::from(minutes) * 60
    }

    pub fn view(&self, pin_lines: &[String]) -> SourceList {
        SourceList {
            lists: self.walk(pin_lines).1,
            check_minutes: self.file.check_minutes,
            error: self.error.clone(),
        }
    }

    pub fn merge_hosts_lines(&self, pin_lines: &[String]) -> Vec<String> {
        self.walk(pin_lines).0
    }

    pub fn ensure_can_add(&self, url: &str) -> Result<String, String> {
        self.ensure_writable()?;
        let url = canonicalize_url(url)?;
        if self.file.lists.len() >= MAX_LISTS {
            return Err("error.listLimit".to_string());
        }
        if self.file.lists.iter().any(|source| source.url == url) {
            return Err("error.listDuplicate".to_string());
        }
        Ok(url)
    }

    pub fn insert(&mut self, title: &str, url: &str, text: &str, etag: &str) -> Result<(), String> {
        let url = self.ensure_can_add(url)?;
        let parsed = parse_list(text)?;
        self.file.lists.push(Source {
            id: Uuid::new_v4().to_string(),
            title: clean_title(title, &parsed.title, &url),
            url,
            enabled: true,
            version: parsed.version,
            etag: clean_etag(etag),
            checked_at: now_secs(),
            error: None,
            entries: parsed.entries,
        });
        self.write()
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        self.ensure_writable()?;
        let before = self.file.lists.len();
        self.file.lists.retain(|source| source.id != id);
        if self.file.lists.len() == before {
            return Err("error.listMissing".to_string());
        }
        self.write()
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        self.ensure_writable()?;
        let source = self
            .file
            .lists
            .iter_mut()
            .find(|source| source.id == id)
            .ok_or_else(|| "error.listMissing".to_string())?;
        source.enabled = enabled;
        self.write()
    }

    pub fn set_interval(&mut self, minutes: u32) -> Result<(), String> {
        self.ensure_writable()?;
        if !INTERVALS.contains(&minutes) {
            return Err("error.listInterval".to_string());
        }
        self.file.check_minutes = minutes;
        self.file.last_check = now_secs();
        self.write()
    }

    pub fn jobs(&self, only: Option<&str>) -> Result<Vec<CheckJob>, String> {
        self.ensure_writable()?;
        if let Some(id) = only {
            let source = self
                .file
                .lists
                .iter()
                .find(|source| source.id == id)
                .ok_or_else(|| "error.listMissing".to_string())?;
            return Ok(vec![job(source)]);
        }
        Ok(self.file.lists.iter().map(job).collect())
    }

    pub fn apply_download(
        &mut self,
        id: &str,
        download: Download,
    ) -> Result<Option<CheckKind>, String> {
        self.ensure_writable()?;
        let Some(source) = self.file.lists.iter_mut().find(|source| source.id == id) else {
            return Ok(None);
        };
        source.checked_at = now_secs();
        let kind = match download {
            Download::NotModified => {
                source.error = None;
                CheckKind::Unchanged
            }
            Download::Failed(error) => {
                source.error = Some(error);
                CheckKind::Failed
            }
            Download::Updated { text, etag } => match parse_list(&text) {
                Ok(parsed) => {
                    let changed = parsed.entries != source.entries;
                    source.version = parsed.version;
                    source.entries = parsed.entries;
                    source.etag = clean_etag(&etag);
                    source.error = None;
                    if changed {
                        CheckKind::Changed
                    } else {
                        CheckKind::Unchanged
                    }
                }
                Err(error) => {
                    source.error = Some(error);
                    CheckKind::Failed
                }
            },
        };
        Ok(Some(kind))
    }

    pub fn mark_full_check(&mut self) {
        self.file.last_check = now_secs();
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.ensure_writable()?;
        self.write()
    }

    fn walk(&self, pin_lines: &[String]) -> (Vec<String>, Vec<SourceView>) {
        let mut hosts = pin_lines.to_vec();
        let mut seen = names_in(pin_lines);
        let mut views = Vec::new();
        for source in &self.file.lists {
            let mut entries = Vec::new();
            for entry in &source.entries {
                let skipped = source.enabled && !seen.insert(entry.name.clone());
                if source.enabled && !skipped {
                    for address in &entry.addresses {
                        hosts.push(format!("{address} {}", entry.name));
                    }
                }
                entries.push(EntryView {
                    name: pins::show_name(&entry.name),
                    addresses: entry.addresses.clone(),
                    skipped,
                });
            }
            views.push(SourceView {
                id: source.id.clone(),
                title: source.title.clone(),
                url: source.url.clone(),
                enabled: source.enabled,
                version: source.version.clone(),
                names: entries.len(),
                checked_at: source.checked_at,
                error: source.error.clone(),
                entries,
            });
        }
        (hosts, views)
    }

    fn ensure_writable(&self) -> Result<(), String> {
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn write(&self) -> Result<(), String> {
        save_file(&self.path, &self.file)
    }
}

fn job(source: &Source) -> CheckJob {
    CheckJob {
        id: source.id.clone(),
        url: source.url.clone(),
        etag: source.etag.clone(),
    }
}

pub fn canonicalize_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 || trimmed.contains([' ', '\t', '\n', '\r', '\\'])
    {
        return Err("error.listUrl".to_string());
    }
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err("error.listUrl".to_string());
    };
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return Err("error.listUrl".to_string());
    }
    if rest.is_empty() || rest.contains('#') {
        return Err("error.listUrl".to_string());
    }
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("error.listUrl".to_string());
    }
    Ok(format!("{}://{rest}", scheme.to_ascii_lowercase()))
}

pub fn download_list(url: &str, etag: &str) -> Download {
    match fetch_list(url, etag) {
        Ok(download) => download,
        Err(error) => Download::Failed(error),
    }
}

fn fetch_list(url: &str, etag: &str) -> Result<Download, String> {
    let url = canonicalize_url(url)?;
    let mut request = list_agent()
        .get(&url)
        .header("User-Agent", "Namevane")
        .header("Accept", "text/plain");
    let etag = clean_etag(etag);
    if !etag.is_empty() {
        request = request.header("If-None-Match", &etag);
    }
    let mut response = match request.call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(304)) => return Ok(Download::NotModified),
        Err(error) => {
            eprintln!("list download: {error}");
            return Err("error.listDownload".to_string());
        }
    };
    canonicalize_url(&response.get_uri().to_string())?;
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(clean_etag)
        .unwrap_or_default();
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            eprintln!("list body: {error}");
            "error.listDownload".to_string()
        })?;
    if bytes.len() > MAX_BYTES {
        return Err("error.listHuge".to_string());
    }
    if bytes.contains(&0) {
        return Err("error.listBinary".to_string());
    }
    let text = String::from_utf8(bytes).map_err(|_| "error.listBinary".to_string())?;
    Ok(Download::Updated { text, etag })
}

fn list_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .max_redirects(5)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent()
}

fn parse_list(text: &str) -> Result<ParsedList, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.len() > MAX_BYTES {
        return Err("error.listHuge".to_string());
    }
    if text.contains('\0') {
        return Err("error.listBinary".to_string());
    }
    let mut version = String::new();
    let mut title = String::new();
    let mut entries: Vec<StoredEntry> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = comment_value(trimmed, "namevane-list:") {
            if !version.is_empty() || !valid_version(rest) {
                return Err(format!("error.listLine|{line_number}"));
            }
            version = rest.to_string();
            continue;
        }
        if let Some(rest) = comment_value(trimmed, "title:") {
            if title.is_empty() {
                title = clean_title(rest, "", "list");
            }
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(address) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            return Err(format!("error.listLine|{line_number}"));
        };
        if parts.next().is_some() {
            return Err(format!("error.listLine|{line_number}"));
        }
        let name = pins::normalize_pin_name(name).map_err(|error| {
            eprintln!("list line {line_number}: {error}");
            format!("error.listLine|{line_number}")
        })?;
        if let Some(entry) = entries.iter_mut().find(|entry| entry.name == name) {
            pins::add_address(&mut entry.addresses, address).map_err(|error| {
                eprintln!("list line {line_number}: {error}");
                format!("error.listLine|{line_number}")
            })?;
        } else {
            if entries.len() >= MAX_NAMES {
                return Err("error.listMany".to_string());
            }
            let mut addresses = Vec::new();
            pins::add_address(&mut addresses, address).map_err(|error| {
                eprintln!("list line {line_number}: {error}");
                format!("error.listLine|{line_number}")
            })?;
            entries.push(StoredEntry { name, addresses });
        }
    }
    if entries.is_empty() {
        return Err("error.listEmpty".to_string());
    }
    if version.is_empty() {
        version = content_version(&entries);
    }
    Ok(ParsedList {
        version,
        title,
        entries,
    })
}

fn comment_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix('#')?.trim();
    let rest = rest.strip_prefix(key)?.trim();
    if rest.is_empty() { None } else { Some(rest) }
}

fn valid_version(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 64
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
}

fn content_version(entries: &[StoredEntry]) -> String {
    let mut canonical = String::new();
    for entry in entries {
        for address in &entry.addresses {
            canonical.push_str(address);
            canonical.push(' ');
            canonical.push_str(&entry.name);
            canonical.push('\n');
        }
    }
    let digest = Sha256::digest(canonical.as_bytes());
    format!("sha256:{}", &hex_encode(&digest)[..12])
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0xf) as usize] as char);
    }
    encoded
}

fn names_in(lines: &[String]) -> HashSet<String> {
    lines
        .iter()
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect()
}

fn clean_title(given: &str, parsed: &str, url: &str) -> String {
    let chosen = if !given.trim().is_empty() {
        given.trim()
    } else if !parsed.trim().is_empty() {
        parsed.trim()
    } else {
        host_of(url)
    };
    let title: String = chosen
        .chars()
        .filter(|ch| *ch != '\n' && *ch != '\r')
        .take(80)
        .collect();
    if title.is_empty() {
        "List".to_string()
    } else {
        title
    }
}

fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let authority = rest.split(['/', '?']).next().unwrap_or(rest);
    if let Some(inside) = authority.strip_prefix('[') {
        return inside.split(']').next().unwrap_or(authority);
    }
    authority.split(':').next().unwrap_or(authority)
}

fn clean_etag(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 256
        || trimmed
            .chars()
            .any(|ch| ch.is_whitespace() || !ch.is_ascii())
    {
        String::new()
    } else {
        trimmed.to_string()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn load_file(path: &Path) -> Result<ListsFile, String> {
    if !path.exists() {
        return Ok(ListsFile::empty());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        eprintln!("read lists: {error}");
        "error.sourcesRead".to_string()
    })?;
    let file: ListsFile = serde_json::from_str(&text).map_err(|error| {
        eprintln!("parse lists: {error}");
        "error.sourcesRead".to_string()
    })?;
    if file.version != VERSION {
        return Err("error.sourcesVersion".to_string());
    }
    Ok(file)
}

fn save_file(path: &Path, file: &ListsFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                eprintln!("create lists dir: {error}");
                "error.sourcesWrite".to_string()
            })?;
        }
    }
    let mut bytes = serde_json::to_vec_pretty(file).map_err(|error| {
        eprintln!("encode lists: {error}");
        "error.sourcesWrite".to_string()
    })?;
    bytes.push(b'\n');
    let temporary = temporary_path(path);
    fs::write(&temporary, bytes).map_err(|error| {
        eprintln!("write lists: {error}");
        "error.sourcesWrite".to_string()
    })?;
    if fs::rename(&temporary, path).is_err() {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&temporary, path).map_err(|error| {
            eprintln!("replace lists: {error}");
            "error.sourcesWrite".to_string()
        })?;
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> ListStore {
        let dir = std::env::temp_dir().join(format!("namevane-lists-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        ListStore::open(dir.join("lists.json"))
    }

    const SAMPLE: &str = "\
# title: Staging
# namevane-list: 3
198.51.100.20 shop.example.com
2001:db8::10 shop.example.com
203.0.113.10 api.example.com
";

    #[test]
    fn parses_a_version_a_title_and_both_address_families() {
        let parsed = parse_list(SAMPLE).unwrap();
        assert_eq!(parsed.version, "3");
        assert_eq!(parsed.title, "Staging");
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(
            parsed.entries[0].addresses,
            vec!["198.51.100.20".to_string(), "2001:db8::10".to_string()]
        );
    }

    #[test]
    fn a_missing_version_is_a_stable_hash_of_the_names() {
        let text = "203.0.113.10 app.example.com\n";
        let first = parse_list(text).unwrap();
        let second = parse_list(text).unwrap();
        assert!(first.version.starts_with("sha256:"));
        assert_eq!(first.version, second.version);
    }

    #[test]
    fn refuses_localhost_a_wildcard_and_a_second_ipv4() {
        assert_eq!(
            parse_list("203.0.113.10 localhost\n").unwrap_err(),
            "error.listLine|1"
        );
        assert_eq!(
            parse_list("203.0.113.10 *.example.com\n").unwrap_err(),
            "error.listLine|1"
        );
        let text = "\
203.0.113.10 app.example.com
203.0.113.11 app.example.com
";
        assert_eq!(parse_list(text).unwrap_err(), "error.listLine|2");
    }

    #[test]
    fn an_empty_list_and_a_bad_address_are_refused() {
        assert_eq!(
            parse_list("# only a comment\n").unwrap_err(),
            "error.listEmpty"
        );
        assert_eq!(
            parse_list("not-an-ip app.example.com\n").unwrap_err(),
            "error.listLine|1"
        );
    }

    #[test]
    fn stores_an_international_name_as_punycode() {
        let parsed = parse_list("203.0.113.10 bücher.example\n").unwrap();
        assert_eq!(parsed.entries[0].name, "xn--bcher-kva.example");
    }

    #[test]
    fn a_pin_and_an_earlier_list_win_the_same_name() {
        let mut saved = store();
        saved
            .insert("", "https://lists.example/a.txt", SAMPLE, "")
            .unwrap();
        saved
            .insert(
                "Other",
                "https://lists.example/b.txt",
                "198.51.100.8 shop.example.com\n203.0.113.8 extra.example.com\n",
                "",
            )
            .unwrap();
        let other = saved.file.lists[1].id.clone();
        saved.set_enabled(&other, false).unwrap();
        let lines = saved.merge_hosts_lines(&["203.0.113.1 shop.example.com".to_string()]);
        assert_eq!(
            lines,
            vec![
                "203.0.113.1 shop.example.com".to_string(),
                "203.0.113.10 api.example.com".to_string(),
            ]
        );

        saved.set_enabled(&other, true).unwrap();
        let lines = saved.merge_hosts_lines(&[]);
        assert!(
            lines
                .iter()
                .any(|line| line == "198.51.100.20 shop.example.com")
        );
        assert!(!lines.iter().any(|line| line.contains("198.51.100.8")));
        assert!(
            lines
                .iter()
                .any(|line| line == "203.0.113.8 extra.example.com")
        );
        let view = saved.view(&["203.0.113.1 shop.example.com".to_string()]);
        let shop = view.lists[0]
            .entries
            .iter()
            .find(|entry| entry.name == "shop.example.com")
            .unwrap();
        assert!(shop.skipped);
    }

    #[test]
    fn a_failed_download_keeps_the_saved_names() {
        let mut saved = store();
        saved
            .insert("", "https://lists.example/a.txt", SAMPLE, "\"one\"")
            .unwrap();
        let id = saved.file.lists[0].id.clone();
        let kind = saved
            .apply_download(&id, Download::Failed("error.listDownload".to_string()))
            .unwrap();
        assert_eq!(kind, Some(CheckKind::Failed));
        assert_eq!(saved.file.lists[0].entries.len(), 2);
        assert_eq!(
            saved.file.lists[0].error.as_deref(),
            Some("error.listDownload")
        );

        let same = saved
            .apply_download(
                &id,
                Download::Updated {
                    text: SAMPLE.to_string(),
                    etag: "\"one\"".to_string(),
                },
            )
            .unwrap();
        assert_eq!(same, Some(CheckKind::Unchanged));
        assert!(saved.file.lists[0].error.is_none());

        let changed = saved
            .apply_download(
                &id,
                Download::Updated {
                    text: "203.0.113.50 shop.example.com\n".to_string(),
                    etag: "\"two\"".to_string(),
                },
            )
            .unwrap();
        assert_eq!(changed, Some(CheckKind::Changed));
        assert_eq!(
            saved.file.lists[0].entries[0].addresses,
            vec!["203.0.113.50".to_string()]
        );
    }

    #[test]
    fn refuses_a_url_that_is_not_http_and_a_duplicate_list() {
        assert_eq!(
            canonicalize_url("file:///etc/hosts").unwrap_err(),
            "error.listUrl"
        );
        assert_eq!(
            canonicalize_url("https://user:secret@lists.example/a.txt").unwrap_err(),
            "error.listUrl"
        );
        assert_eq!(
            canonicalize_url("HTTPS://lists.example/a.txt").unwrap(),
            "https://lists.example/a.txt"
        );
        let mut saved = store();
        saved
            .insert("Staging", "https://lists.example/a.txt", SAMPLE, "")
            .unwrap();
        assert_eq!(
            saved
                .insert("Again", "https://lists.example/a.txt", SAMPLE, "")
                .unwrap_err(),
            "error.listDuplicate"
        );
        assert_eq!(saved.set_interval(7).unwrap_err(), "error.listInterval");
        assert!(saved.file.lists[0].title == "Staging");
    }

    #[test]
    fn a_blank_title_uses_the_title_in_the_file() {
        let mut saved = store();
        saved
            .insert("  ", "https://lists.example/a.txt", SAMPLE, "")
            .unwrap();
        assert_eq!(saved.file.lists[0].title, "Staging");
    }
}

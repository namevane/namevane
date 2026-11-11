use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinsFile {
    pub version: u32,
    #[serde(rename = "appliedRevision")]
    pub applied_revision: u64,
    pub pins: Vec<Pin>,
}

impl PinsFile {
    pub fn empty() -> Self {
        Self {
            version: VERSION,
            applied_revision: 0,
            pins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pin {
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertPin {
    pub id: Option<String>,
    pub name: String,
    pub target: String,
    #[serde(default)]
    pub other_address: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PinView {
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PinList {
    pub pins: Vec<PinView>,
    pub error: Option<String>,
}

pub struct PinStore {
    path: PathBuf,
    file: PinsFile,
    error: Option<String>,
}

impl PinStore {
    pub fn open(path: PathBuf) -> Self {
        match load_file(&path) {
            Ok(mut file) => {
                let before = file.pins.len();
                file.pins.retain(|pin| !pin.addresses.is_empty());
                let store = Self {
                    path,
                    file,
                    error: None,
                };
                if store.file.pins.len() != before {
                    let _ = store.write();
                }
                store
            }
            Err(error) => Self {
                path,
                file: PinsFile::empty(),
                error: Some(error),
            },
        }
    }

    pub fn directory(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }

    pub fn hosts_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for pin in &self.file.pins {
            if !pin.enabled {
                continue;
            }
            for address in &pin.addresses {
                lines.push(format!("{address} {}", pin.name));
            }
        }
        lines
    }

    pub fn mark_applied(&mut self) -> Result<(), String> {
        self.ensure_writable()?;
        self.file.applied_revision = self.file.applied_revision.saturating_add(1);
        self.write()
    }

    pub fn list(&self) -> PinList {
        PinList {
            pins: self.file.pins.iter().map(PinView::from).collect(),
            error: self.error.clone(),
        }
    }

    pub fn upsert(&mut self, input: UpsertPin) -> Result<PinView, String> {
        self.ensure_writable()?;
        let pin = validate_pin(input, &self.file.pins)?;
        if let Some(existing) = self.file.pins.iter_mut().find(|saved| saved.id == pin.id) {
            *existing = pin.clone();
        } else {
            self.file.pins.push(pin.clone());
        }
        self.write()?;
        Ok(PinView::from(&pin))
    }

    pub fn export_text(&self) -> Result<String, String> {
        self.ensure_writable()?;
        let mut bytes = serde_json::to_vec_pretty(&self.file)
            .map_err(|_| "error.listSave".to_string())?;
        bytes.push(b'\n');
        String::from_utf8(bytes).map_err(|_| "error.listSave".to_string())
    }

    pub fn import_text(&mut self, text: &str) -> Result<ImportReport, String> {
        self.ensure_writable()?;
        let incoming: PinsFile = serde_json::from_str(text)
            .map_err(|_| "error.importFormat".to_string())?;
        if incoming.version != VERSION {
            return Err("error.importVersion".to_string());
        }
        if incoming.pins.is_empty() {
            return Err("error.importEmpty".to_string());
        }
        let mut next = self.file.clone();
        let mut added = 0;
        let mut skipped = 0;
        let mut without_address = 0;
        for pin in incoming.pins {
            if pin.addresses.is_empty() {
                without_address += 1;
                continue;
            }
            if next
                .pins
                .iter()
                .any(|saved| saved.id == pin.id || saved.name == pin.name)
            {
                skipped += 1;
                continue;
            }
            let target = pin.addresses.first().cloned().unwrap_or_default();
            let other = pin.addresses.get(1).cloned().unwrap_or_default();
            let saved = validate_pin(
                UpsertPin {
                    id: None,
                    name: pin.name,
                    target,
                    other_address: other,
                    enabled: pin.enabled,
                },
                &next.pins,
            )?;
            next.pins.push(saved);
            added += 1;
        }
        if added == 0 {
            if without_address > 0 && skipped == 0 {
                return Err("error.importEmpty".to_string());
            }
            return Err("error.importNone".to_string());
        }
        self.file = next;
        self.write()?;
        Ok(ImportReport { added, skipped })
    }

    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.ensure_writable()?;
        let before = self.file.pins.len();
        self.file.pins.retain(|pin| pin.id != id);
        if self.file.pins.len() == before {
            return Err("error.missing".to_string());
        }
        self.write()
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<PinView, String> {
        self.ensure_writable()?;
        let pin = self
            .file
            .pins
            .iter_mut()
            .find(|pin| pin.id == id)
            .ok_or_else(|| "error.missing".to_string())?;
        pin.enabled = enabled;
        let view = PinView::from(&*pin);
        self.write()?;
        Ok(view)
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

impl From<&Pin> for PinView {
    fn from(pin: &Pin) -> Self {
        Self {
            id: pin.id.clone(),
            name: display_name(&pin.name),
            addresses: pin.addresses.clone(),
            enabled: pin.enabled,
        }
    }
}

fn validate_pin(input: UpsertPin, existing: &[Pin]) -> Result<Pin, String> {
    let name = normalize_name(&input.name, "error.nameRequired")?;
    let addresses = resolve_addresses(&input.target, &input.other_address)?;
    if existing
        .iter()
        .any(|pin| pin.name == name && input.id.as_deref() != Some(pin.id.as_str()))
    {
        return Err("error.duplicate".to_string());
    }
    let id = match input.id {
        Some(id) if existing.iter().any(|pin| pin.id == id) => id,
        Some(_) => return Err("error.missing".to_string()),
        None => Uuid::new_v4().to_string(),
    };
    Ok(Pin {
        id,
        name,
        addresses,
        enabled: input.enabled,
    })
}

fn resolve_addresses(target: &str, other: &str) -> Result<Vec<String>, String> {
    let mut addresses = vec![parse_address(target)?];
    if !other.trim().is_empty() {
        addresses.push(parse_address(other)?);
    }
    let v4 = addresses.iter().filter(|ip| ip.is_ipv4()).count();
    let v6 = addresses.iter().filter(|ip| ip.is_ipv6()).count();
    if v4 > 1 || v6 > 1 {
        return Err("error.oneEach".to_string());
    }
    let mut stored = Vec::new();
    if let Some(ip) = addresses.iter().find(|ip| ip.is_ipv4()) {
        stored.push(ip.to_string());
    }
    if let Some(ip) = addresses.iter().find(|ip| ip.is_ipv6()) {
        stored.push(ip.to_string());
    }
    Ok(stored)
}

fn parse_address(raw: &str) -> Result<IpAddr, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("error.targetRequired".to_string());
    }
    trimmed
        .parse::<IpAddr>()
        .map_err(|_| "error.badAddress".to_string())
}

fn normalize_name(raw: &str, empty_message: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(empty_message.to_string());
    }
    if trimmed.contains("://") || trimmed.contains('/') {
        return Err("error.noScheme".to_string());
    }
    if trimmed.contains('*') {
        return Err("error.wildcard".to_string());
    }
    let without_dot = trimmed.strip_suffix('.').unwrap_or(trimmed);
    if without_dot.is_empty() || without_dot.parse::<IpAddr>().is_ok() {
        return Err("error.nameNotAddress".to_string());
    }
    let ascii = idna::domain_to_ascii_strict(without_dot)
        .map_err(|_| "error.badName".to_string())?;
    if is_localhost(&ascii) {
        return Err("error.localhost".to_string());
    }
    Ok(ascii)
}

fn is_localhost(name: &str) -> bool {
    name == "localhost" || name == "localhost.localdomain" || name.ends_with(".localhost")
}

fn display_name(ascii: &str) -> String {
    let (unicode, result) = idna::domain_to_unicode(ascii);
    if result.is_ok() {
        unicode
    } else {
        ascii.to_string()
    }
}

pub fn load_file(path: &Path) -> Result<PinsFile, String> {
    if !path.exists() {
        return Ok(PinsFile::empty());
    }
    let text = fs::read_to_string(path).map_err(|err| {
        eprintln!("read pins: {err}");
        "error.listRead".to_string()
    })?;
    let file: PinsFile = serde_json::from_str(&text).map_err(|err| {
        eprintln!("parse pins: {err}");
        "error.listRead".to_string()
    })?;
    if file.version != VERSION {
        return Err("error.listVersion".to_string());
    }
    Ok(file)
}

pub fn save_file(path: &Path, file: &PinsFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                eprintln!("create pins dir: {err}");
                "error.listWrite".to_string()
            })?;
        }
    }
    let mut bytes = serde_json::to_vec_pretty(file).map_err(|err| {
        eprintln!("encode pins: {err}");
        "error.listWrite".to_string()
    })?;
    bytes.push(b'\n');
    let temporary = temporary_path(path);
    fs::write(&temporary, bytes).map_err(|err| {
        eprintln!("write pins: {err}");
        "error.listWrite".to_string()
    })?;
    replace_file(&temporary, path).map_err(|err| {
        eprintln!("replace pins: {err}");
        "error.listWrite".to_string()
    })
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if to.exists() {
        fs::remove_file(to)?;
    }
    fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (PathBuf, PinStore) {
        let dir = std::env::temp_dir().join(format!("namevane-pins-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pins.json");
        (dir, PinStore::open(path))
    }

    fn add(store: &mut PinStore, name: &str, target: &str, other: &str) -> Result<PinView, String> {
        store.upsert(UpsertPin {
            id: None,
            name: name.to_string(),
            target: target.to_string(),
            other_address: other.to_string(),
            enabled: true,
        })
    }

    #[test]
    fn refuses_an_empty_name_and_a_bad_address() {
        let (_dir, mut store) = temp_store();
        assert_eq!(
            add(&mut store, "  ", "203.0.113.10", "").unwrap_err(),
            "error.nameRequired"
        );
        assert_eq!(
            add(&mut store, "app.example.com", "   ", "").unwrap_err(),
            "error.targetRequired"
        );
        assert_eq!(
            add(&mut store, "app.example.com", "999.1.1.1", "").unwrap_err(),
            "error.badAddress"
        );
        assert!(store.list().pins.is_empty());
    }

    #[test]
    fn refuses_localhost_and_a_wildcard() {
        let (_dir, mut store) = temp_store();
        assert_eq!(
            add(&mut store, "localhost", "203.0.113.10", "").unwrap_err(),
            "error.localhost"
        );
        assert_eq!(
            add(&mut store, "*.example.com", "203.0.113.10", "").unwrap_err(),
            "error.wildcard"
        );
        assert_eq!(
            add(&mut store, "app.example.com", "localhost", "").unwrap_err(),
            "error.badAddress"
        );
    }

    #[test]
    fn saves_an_address_and_a_second_address() {
        let (dir, mut store) = temp_store();
        let address = add(&mut store, "App.Example.com", "203.0.113.10", "").unwrap();
        assert_eq!(address.name, "app.example.com");
        assert_eq!(address.addresses, vec!["203.0.113.10".to_string()]);

        let both = add(
            &mut store,
            "both.example.com",
            "2001:db8::10",
            "203.0.113.20",
        )
        .unwrap();
        assert_eq!(
            both.addresses,
            vec!["203.0.113.20".to_string(), "2001:db8::10".to_string()]
        );
        assert_eq!(
            store.hosts_lines(),
            vec![
                "203.0.113.10 app.example.com".to_string(),
                "203.0.113.20 both.example.com".to_string(),
                "2001:db8::10 both.example.com".to_string(),
            ]
        );

        let loaded = PinStore::open(dir.join("pins.json"));
        assert_eq!(loaded.list().pins.len(), 2);
        assert_eq!(loaded.file.applied_revision, 0);
    }

    #[test]
    fn a_saved_name_without_an_address_is_dropped() {
        let dir = std::env::temp_dir().join(format!("namevane-pins-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pins.json");
        fs::write(
            &path,
            r#"{
  "version": 1,
  "appliedRevision": 0,
  "pins": [
    {"id":"1","name":"app.example.com","addresses":["203.0.113.10"],"enabled":true},
    {"id":"2","name":"alias.example.com","addresses":[],"host":"other.example.com","enabled":true}
  ]
}
"#,
        )
        .unwrap();
        let store = PinStore::open(path.clone());
        assert_eq!(store.list().pins.len(), 1);
        assert_eq!(store.list().pins[0].name, "app.example.com");
        let text = fs::read_to_string(path).unwrap();
        assert!(!text.contains("alias.example.com"));
        assert!(!text.contains("host"));
    }

    #[test]
    fn stores_an_international_name_as_punycode_and_shows_unicode() {
        let (_dir, mut store) = temp_store();
        let pin = add(&mut store, "bücher.example", "203.0.113.10", "").unwrap();
        assert_eq!(pin.name, "bücher.example");
        assert_eq!(store.file.pins[0].name, "xn--bcher-kva.example");
    }

    #[test]
    fn updates_and_removes_a_pin_and_turns_it_off() {
        let (dir, mut store) = temp_store();
        let pin = add(&mut store, "app.example.com", "203.0.113.10", "").unwrap();
        let updated = store
            .upsert(UpsertPin {
                id: Some(pin.id.clone()),
                name: "app.example.com".to_string(),
                target: "198.51.100.20".to_string(),
                other_address: String::new(),
                enabled: true,
            })
            .unwrap();
        assert_eq!(updated.addresses, vec!["198.51.100.20".to_string()]);
        assert_eq!(store.list().pins.len(), 1);

        let off = store.set_enabled(&pin.id, false).unwrap();
        assert!(!off.enabled);

        store.delete(&pin.id).unwrap();
        assert!(store.list().pins.is_empty());
        let loaded = load_file(&dir.join("pins.json")).unwrap();
        assert!(loaded.pins.is_empty());
    }

    #[test]
    fn export_and_import_round_trip_and_skip_names_already_saved() {
        let (_dir, mut store) = temp_store();
        add(&mut store, "app.example.com", "203.0.113.10", "").unwrap();
        let first = store.export_text().unwrap();
        add(&mut store, "other.example.com", "198.51.100.8", "").unwrap();
        let both = store.export_text().unwrap();
        assert_eq!(
            store.import_text(&first).unwrap_err(),
            "error.importNone"
        );

        let mut fresh = PinStore::open(
            std::env::temp_dir().join(format!("namevane-import-{}", Uuid::new_v4())),
        );
        let report = fresh.import_text(&first).unwrap();
        assert_eq!(report.added, 1);
        let report = fresh.import_text(&both).unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(fresh.list().pins.len(), 2);
    }

    #[test]
    fn refuses_a_duplicate_name_and_two_addresses_of_one_kind() {
        let (_dir, mut store) = temp_store();
        add(&mut store, "app.example.com", "203.0.113.10", "").unwrap();
        assert_eq!(
            add(&mut store, "APP.example.com", "198.51.100.8", "").unwrap_err(),
            "error.duplicate"
        );
        assert_eq!(
            add(
                &mut store,
                "other.example.com",
                "203.0.113.10",
                "198.51.100.8"
            )
            .unwrap_err(),
            "error.oneEach"
        );
        assert_eq!(
            add(&mut store, "loop.example.com", "loop.example.com", "").unwrap_err(),
            "error.badAddress"
        );
    }

    #[test]
    fn does_not_replace_a_list_that_cannot_be_read() {
        let dir = std::env::temp_dir().join(format!("namevane-pins-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pins.json");
        fs::write(&path, "{not json").unwrap();
        let mut store = PinStore::open(path.clone());
        assert_eq!(
            store.list().error.as_deref(),
            Some("error.listRead")
        );
        assert!(add(&mut store, "app.example.com", "203.0.113.10", "").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{not json");
    }

    #[test]
    fn saving_a_pin_leaves_the_real_hosts_file_unchanged() {
        let hosts = if cfg!(windows) {
            PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
        } else {
            PathBuf::from("/etc/hosts")
        };
        let before = fs::read(&hosts).expect("hosts file");
        let (_dir, mut store) = temp_store();
        add(&mut store, "app.example.com", "203.0.113.10", "").unwrap();
        let after = fs::read(&hosts).expect("hosts file");
        assert_eq!(before, after);
    }
}

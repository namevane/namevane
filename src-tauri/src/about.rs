use std::fs;
use std::io::{Read, Write};
use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

pub const REPOSITORY: &str = "https://github.com/namevane/namevane";
const RELEASES: &str = "https://api.github.com/repos/namevane/namevane/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct About {
    pub name: &'static str,
    pub maker: &'static str,
    pub version: String,
    pub repository: &'static str,
    pub releases: &'static str,
    pub update: UpdateInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateInfo {
    None,
    Current,
    Available { version: String, url: String },
    Unknown { message: String },
}

pub fn about(version: &str) -> About {
    About {
        name: "Namevane",
        maker: "RumenX",
        version: version.to_string(),
        repository: REPOSITORY,
        releases: "https://github.com/namevane/namevane/releases",
        update: check_for_update(version),
    }
}

pub fn open_project_page(url: &str) -> Result<(), String> {
    if !is_project_url(url) {
        return Err("error.notRepository".to_string());
    }
    open_url(url)
}

fn check_for_update(current: &str) -> UpdateInfo {
    let Some(current_version) = parse_version(current) else {
        return UpdateInfo::Unknown {
            message: "error.versionUnreadable".to_string(),
        };
    };
    match fetch_latest_release() {
        Ok(None) => UpdateInfo::None,
        Ok(Some(release)) => match parse_version(&release.tag_name) {
            Some(latest)
                if is_newer(&latest, &current_version) && is_project_url(&release.html_url) =>
            {
                UpdateInfo::Available {
                    version: latest.display(),
                    url: release.html_url,
                }
            }
            Some(_) => UpdateInfo::Current,
            None => UpdateInfo::Unknown {
                message: "error.releaseUnreadable".to_string(),
            },
        },
        Err(message) => UpdateInfo::Unknown { message },
    }
}

struct Release {
    tag_name: String,
    html_url: String,
    assets: Vec<Asset>,
}

struct Asset {
    name: String,
    url: String,
}

pub fn install_update(version: &str) -> Result<(), String> {
    let Some(current) = parse_version(version) else {
        return Err("error.versionUnreadable".to_string());
    };
    let release = fetch_latest_release()?.ok_or_else(|| "error.noRelease".to_string())?;
    let Some(latest) = parse_version(&release.tag_name) else {
        return Err("error.releaseUnreadable".to_string());
    };
    if !is_newer(&latest, &current) {
        return Err("error.latest".to_string());
    }
    let disk = preferred_disk_image(&release.assets, std::env::consts::ARCH)
        .ok_or_else(|| "error.noInstaller".to_string())?;
    let sums = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS")
        .ok_or_else(|| "error.noChecksum".to_string())?;
    if !is_release_download(&disk.url) || !is_release_download(&sums.url) {
        return Err("error.badDownload".to_string());
    }

    let directory = std::env::temp_dir().join(format!("namevane-update-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).map_err(|_| "error.updateSave".to_string())?;
    let disk_path = directory.join(&disk.name);
    let sums_text = download_text(&sums.url)?;
    download_file(&disk.url, &disk_path)?;
    let expected =
        checksum_for(&sums_text, &disk.name).ok_or_else(|| "error.checksumMissing".to_string())?;
    let actual = sha256_file(&disk_path)?;
    if actual != expected {
        let _ = fs::remove_dir_all(&directory);
        return Err("error.checksumMismatch".to_string());
    }
    stage_and_swap(&disk_path, &directory)
}

fn fetch_latest_release() -> Result<Option<Release>, String> {
    let mut response = match http_agent(Duration::from_secs(8))
        .get(RELEASES)
        .header("User-Agent", "Namevane")
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(err) => {
            eprintln!("release check: {err}");
            return Err("error.releaseCheck".to_string());
        }
    };
    let body: serde_json::Value = response.body_mut().read_json().map_err(|err| {
        eprintln!("release check body: {err}");
        "error.releaseCheck".to_string()
    })?;
    let tag_name = body
        .get("tag_name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let html_url = body
        .get("html_url")
        .and_then(|value| value.as_str())
        .unwrap_or(REPOSITORY)
        .to_string();
    let assets = body
        .get("assets")
        .and_then(|value| value.as_array())
        .map(|assets| {
            assets
                .iter()
                .filter_map(|asset| {
                    Some(Asset {
                        name: asset.get("name")?.as_str()?.to_string(),
                        url: asset.get("browser_download_url")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if tag_name.is_empty() {
        return Err("error.releaseCheck".to_string());
    }
    Ok(Some(Release {
        tag_name,
        html_url,
        assets,
    }))
}

fn preferred_disk_image<'a>(assets: &'a [Asset], arch: &str) -> Option<&'a Asset> {
    let markers: &[&str] = match arch {
        "aarch64" => &["aarch64"],
        "x86_64" => &["x64", "x86_64"],
        _ => return None,
    };
    assets.iter().find(|asset| {
        let name = asset.name.to_ascii_lowercase();
        name.ends_with(".dmg") && markers.iter().any(|marker| name.contains(marker))
    })
}

fn checksum_for(sums: &str, file_name: &str) -> Option<String> {
    for line in sums.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?.to_ascii_lowercase();
        let name = parts.next()?;
        if name == file_name || name.ends_with(&format!("/{file_name}")) {
            return Some(hash);
        }
    }
    None
}

fn is_release_download(url: &str) -> bool {
    url.starts_with("https://github.com/namevane/namevane/releases/download/")
        && !url.contains(['\n', '\r', ' '])
}

fn download_text(url: &str) -> Result<String, String> {
    let mut bytes = Vec::new();
    download_to(url, &mut bytes)?;
    String::from_utf8(bytes).map_err(|_| "error.checksumRead".to_string())
}

fn download_file(url: &str, path: &Path) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|_| "error.updateSave".to_string())?;
    let mut bytes = Vec::new();
    download_to(url, &mut bytes)?;
    file.write_all(&bytes)
        .map_err(|_| "error.updateSave".to_string())
}

fn download_to(url: &str, destination: &mut Vec<u8>) -> Result<(), String> {
    if !is_release_download(url) {
        return Err("error.badDownload".to_string());
    }
    let mut response = http_agent(Duration::from_secs(120))
        .get(url)
        .header("User-Agent", "Namevane")
        .call()
        .map_err(|err| {
            eprintln!("update download: {err}");
            "error.updateDownload".to_string()
        })?;
    response
        .body_mut()
        .as_reader()
        .take(200 * 1024 * 1024)
        .read_to_end(destination)
        .map_err(|_| "error.updateDownload".to_string())?;
    Ok(())
}

fn http_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|_| "error.updateRead".to_string())?;
    Ok(hex_encode(&Sha256::digest(bytes)))
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

fn stage_and_swap(disk_image: &Path, directory: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        swap_macos_app(disk_image, directory)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (disk_image, directory);
        Err("error.manualInstall".to_string())
    }
}

#[cfg(target_os = "macos")]
fn swap_macos_app(disk_image: &Path, directory: &Path) -> Result<(), String> {
    let destination = installed_app()?;
    let mount = directory.join("mount");
    let staged = directory.join("Namevane.app");
    let attached = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(disk_image)
        .status()
        .map_err(|_| "error.installerOpen".to_string())?
        .success();
    if !attached {
        return Err("error.installerOpen".to_string());
    }
    let source = find_app(&mount).ok_or_else(|| {
        let _ = detach(&mount);
        "error.installerMissing".to_string()
    })?;
    let copied = Command::new("ditto")
        .arg(&source)
        .arg(&staged)
        .status()
        .map_err(|_| "error.updatePrepare".to_string());
    let _ = detach(&mount);
    if !copied?.success() || !staged.is_dir() {
        return Err("error.updatePrepare".to_string());
    }
    spawn_replacer(&staged, &destination)
}

#[cfg(target_os = "macos")]
fn installed_app() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|_| "error.installFirst".to_string())?;
    let bundle = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| "error.installFirst".to_string())?;
    if bundle.file_name().and_then(|name| name.to_str()) != Some("Namevane.app") {
        return Err("error.installFirst".to_string());
    }
    Ok(bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
fn find_app(mount: &Path) -> Option<PathBuf> {
    let direct = mount.join("Namevane.app");
    if direct.is_dir() {
        return Some(direct);
    }
    let entries = fs::read_dir(mount).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join("Namevane.app");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn detach(mount: &Path) -> Result<(), String> {
    Command::new("hdiutil")
        .args(["detach"])
        .arg(mount)
        .status()
        .map_err(|_| "error.installerClose".to_string())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_replacer(staged: &Path, destination: &Path) -> Result<(), String> {
    let script = staged.parent().unwrap_or(staged).join("replace.sh");
    fs::write(
        &script,
        r#"#!/bin/bash
set -euo pipefail
pid="$1"
src="$2"
dest="$3"
while kill -0 "$pid" 2>/dev/null; do
  sleep 0.2
done
rm -rf "$dest"
ditto "$src" "$dest"
open "$dest"
rm -rf "$(dirname "$src")"
"#,
    )
    .map_err(|_| "error.updatePrepare".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Command::new("bash")
            .arg(&script)
            .arg(std::process::id().to_string())
            .arg(staged)
            .arg(destination)
            .process_group(0)
            .spawn()
            .map_err(|_| "error.updateStart".to_string())?;
    }
    Ok(())
}

fn is_project_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix(REPOSITORY) else {
        return false;
    };
    !rest.contains(['\n', '\r', ' '])
        && (rest.is_empty()
            || rest.starts_with('/')
            || rest.starts_with('?')
            || rest.starts_with('#'))
}

fn open_url(url: &str) -> Result<(), String> {
    let status = open_command(url)
        .status()
        .map_err(|_| "error.browser".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("error.browser".to_string())
    }
}

fn open_command(url: &str) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(url);
        command
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Version {
    parts: Vec<u64>,
    pre: bool,
}

impl Version {
    fn display(&self) -> String {
        let numbers = self
            .parts
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".");
        if self.pre {
            format!("{numbers} (pre-release)")
        } else {
            numbers
        }
    }
}

fn parse_version(raw: &str) -> Option<Version> {
    let raw = raw.trim().trim_start_matches('v');
    if raw.is_empty() {
        return None;
    }
    let (numbers, pre) = match raw.split_once('-') {
        Some((numbers, _)) => (numbers, true),
        None => (raw, false),
    };
    if numbers.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for piece in numbers.split('.') {
        if piece.is_empty() {
            return None;
        }
        parts.push(piece.parse().ok()?);
    }
    Some(Version { parts, pre })
}

fn is_newer(latest: &Version, current: &Version) -> bool {
    let len = latest.parts.len().max(current.parts.len());
    for index in 0..len {
        let latest_part = latest.parts.get(index).copied().unwrap_or(0);
        let current_part = current.parts.get(index).copied().unwrap_or(0);
        if latest_part > current_part {
            return true;
        }
        if latest_part < current_part {
            return false;
        }
    }
    !latest.pre && current.pre
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_is_newer_only_when_its_numbers_are_greater() {
        let current = parse_version("0.0.0").unwrap();
        assert!(is_newer(&parse_version("v0.1.0").unwrap(), &current));
        assert!(!is_newer(&parse_version("0.0.0").unwrap(), &current));
        assert!(!is_newer(&parse_version("v0.0.0-rc.1").unwrap(), &current));
        assert!(is_newer(
            &parse_version("1.0.0").unwrap(),
            &parse_version("1.0.0-rc.1").unwrap()
        ));
    }

    #[test]
    fn the_installer_must_match_this_computer_and_its_checksum() {
        let assets = vec![
            Asset {
                name: "Namevane_0.1.1_x64.dmg".to_string(),
                url: "https://github.com/namevane/namevane/releases/download/v0.1.1/Namevane_0.1.1_x64.dmg".to_string(),
            },
            Asset {
                name: "Namevane_0.1.1_aarch64.dmg".to_string(),
                url: "https://github.com/namevane/namevane/releases/download/v0.1.1/Namevane_0.1.1_aarch64.dmg".to_string(),
            },
        ];
        assert_eq!(
            preferred_disk_image(&assets, "aarch64").map(|asset| asset.name.as_str()),
            Some("Namevane_0.1.1_aarch64.dmg")
        );
        assert_eq!(
            checksum_for(
                "abc123  Namevane_0.1.1_aarch64.dmg\n",
                "Namevane_0.1.1_aarch64.dmg"
            )
            .as_deref(),
            Some("abc123")
        );
        assert!(!is_release_download("https://example.com/Namevane.dmg"));
    }

    #[test]
    fn only_the_namevane_repository_may_be_opened() {
        assert!(is_project_url(REPOSITORY));
        assert!(is_project_url(
            "https://github.com/namevane/namevane/releases/tag/v0.1.0"
        ));
        assert!(!is_project_url(
            "https://github.com/namevane/namevane-other"
        ));
        assert!(!is_project_url("https://example.com"));
    }
}

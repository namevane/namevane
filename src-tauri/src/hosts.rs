use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const START: &str = "# >>> namevane >>>";
pub const END: &str = "# <<< namevane <<<";
const MAX_HOSTS_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct CommandFile {
    lines: Vec<String>,
}

pub fn render(original: &str, lines: &[String]) -> Result<String, String> {
    for line in lines {
        if !valid_pin_line(line) {
            return Err("error.hostsCorrupt".to_string());
        }
    }
    let (mut rows, ended_with_newline) = split_lines(original);
    let block = find_block(&rows)?;
    let replacement = block_rows(lines);
    match block {
        Some((start, end)) => {
            rows.splice(start..end, replacement);
        }
        None => {
            if !lines.is_empty() {
                if !rows.is_empty() && !rows.last().is_some_and(|row| row.is_empty()) {
                    rows.push(String::new());
                }
                rows.extend(replacement);
            }
        }
    }
    Ok(join_lines(&rows, ended_with_newline || !lines.is_empty()))
}

pub fn in_sync(original: &str, lines: &[String]) -> Result<bool, String> {
    Ok(render(original, lines)? == original)
}

pub fn has_block(original: &str) -> Result<bool, String> {
    let (rows, _) = split_lines(original);
    Ok(find_block(&rows)?.is_some())
}

pub fn valid_pin_line(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(address) = parts.next() else {
        return false;
    };
    let Some(name) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    address.parse::<IpAddr>().is_ok()
        && !name.contains('*')
        && !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-')
}

pub fn read_hosts(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|_| "error.hostsRead".to_string())?;
    if bytes.len() > MAX_HOSTS_BYTES {
        return Err("error.hostsHuge".to_string());
    }
    if bytes.contains(&0) {
        return Err("error.hostsBinary".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "error.hostsBinary".to_string())
}

pub fn backup_hosts(contents: &str, directory: &Path) -> Result<(), String> {
    let folder = directory.join("backups");
    fs::create_dir_all(&folder).map_err(|_| "error.hostsBackup".to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let path = folder.join(format!("hosts-{stamp}"));
    fs::write(path, contents).map_err(|_| "error.hostsBackup".to_string())
}

pub fn apply_privileged(command_path: &Path) -> i32 {
    match apply_command(command_path) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn apply_command(command_path: &Path) -> Result<(), String> {
    let command: CommandFile = serde_json::from_reader(
        fs::File::open(command_path).map_err(|_| "error.hostsWrite".to_string())?,
    )
    .map_err(|_| "error.hostsWrite".to_string())?;
    for line in &command.lines {
        if !valid_pin_line(line) {
            return Err("error.hostsCorrupt".to_string());
        }
    }
    let hosts_path = Path::new("/etc/hosts");
    let current = read_hosts(hosts_path)?;
    let next = render(&current, &command.lines)?;
    write_hosts(hosts_path, &next)?;
    flush_dns()
}

fn write_hosts(path: &Path, contents: &str) -> Result<(), String> {
    let temporary = PathBuf::from("/etc/hosts.namevane.tmp");
    {
        let mut file = fs::File::create(&temporary).map_err(|_| "error.hostsWrite".to_string())?;
        file.write_all(contents.as_bytes())
            .map_err(|_| "error.hostsWrite".to_string())?;
        let _ = file.sync_all();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(0o644));
    }
    fs::rename(&temporary, path).map_err(|_| "error.hostsWrite".to_string())
}

fn flush_dns() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("dscacheutil")
            .args(["-flushcache"])
            .status();
        let _ = std::process::Command::new("killall")
            .args(["-HUP", "mDNSResponder"])
            .status();
    }
    Ok(())
}

pub fn prompt_apply(lines: &[String], command_path: &Path) -> Result<(), String> {
    write_command(command_path, lines)?;
    #[cfg(target_os = "macos")]
    {
        let executable = std::env::current_exe().map_err(|_| "error.hostsWrite".to_string())?;
        let source = format!(
            "do shell script quoted form of \"{}\" & \" --apply-hosts \" & quoted form of \"{}\" with administrator privileges",
            executable.display().to_string().replace('"', ""),
            command_path.display().to_string().replace('"', "")
        );
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(source)
            .output()
            .map_err(|_| "error.hostsWrite".to_string())?;
        if output.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("User canceled") || detail.contains("(-128)") {
            return Err("hosts.cancelled".to_string());
        }
        eprintln!("apply hosts: {detail}");
        return Err("error.hostsWrite".to_string());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = lines;
        Err("error.hostsUnsupported".to_string())
    }
}

fn write_command(path: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "error.hostsWrite".to_string())?;
    }
    let bytes = serde_json::to_vec(&CommandFile {
        lines: lines.to_vec(),
    })
    .map_err(|_| "error.hostsWrite".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| "error.hostsWrite".to_string())?;
        file.write_all(&bytes)
            .map_err(|_| "error.hostsWrite".to_string())?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes).map_err(|_| "error.hostsWrite".to_string())
    }
}

fn split_lines(original: &str) -> (Vec<String>, bool) {
    if original.is_empty() {
        return (Vec::new(), false);
    }
    let ended = original.ends_with('\n');
    let body = original.strip_suffix('\n').unwrap_or(original);
    (body.split('\n').map(str::to_string).collect(), ended)
}

fn join_lines(rows: &[String], ended_with_newline: bool) -> String {
    if rows.is_empty() {
        return if ended_with_newline {
            "\n".to_string()
        } else {
            String::new()
        };
    }
    let mut text = rows.join("\n");
    if ended_with_newline {
        text.push('\n');
    }
    text
}

fn find_block(rows: &[String]) -> Result<Option<(usize, usize)>, String> {
    let start = rows
        .iter()
        .position(|row| row.trim_end_matches('\r') == START);
    let end = rows
        .iter()
        .position(|row| row.trim_end_matches('\r') == END);
    match (start, end) {
        (Some(start), Some(end)) if start < end => Ok(Some((start, end + 1))),
        (None, None) => Ok(None),
        _ => Err("error.hostsCorrupt".to_string()),
    }
}

fn block_rows(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let mut rows = vec![START.to_string()];
    rows.extend(lines.iter().cloned());
    rows.push(END.to_string());
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
##
# Host Database
#
127.0.0.1 localhost
255.255.255.255 broadcasthost
::1 localhost

# printer
10.0.0.5 printer.local
";

    #[test]
    fn a_missing_block_is_appended_and_other_lines_stay() {
        let rendered = render(FIXTURE, &["10.10.10.122 test.example.com".to_string()]).unwrap();
        assert!(rendered.contains("127.0.0.1 localhost"));
        assert!(rendered.contains("10.0.0.5 printer.local"));
        assert!(rendered.contains("# printer"));
        assert_eq!(rendered.matches(START).count(), 1);
        assert!(rendered.contains("10.10.10.122 test.example.com"));
        let again = render(&rendered, &["10.10.10.122 test.example.com".to_string()]).unwrap();
        assert_eq!(again.matches(START).count(), 1);
        assert_eq!(again, rendered);
    }

    #[test]
    fn an_existing_block_is_replaced_once() {
        let first = render(
            FIXTURE,
            &[
                "203.0.113.10 app.example.com".to_string(),
                "2001:db8::10 app.example.com".to_string(),
            ],
        )
        .unwrap();
        let second = render(&first, &["198.51.100.8 app.example.com".to_string()]).unwrap();
        assert_eq!(second.matches(START).count(), 1);
        assert!(!second.contains("203.0.113.10"));
        assert!(second.contains("198.51.100.8 app.example.com"));
        assert!(second.contains("10.0.0.5 printer.local"));
    }

    #[test]
    fn turning_off_removes_the_block() {
        let applied = render(FIXTURE, &["10.1.1.1 only.example.com".to_string()]).unwrap();
        let cleared = render(&applied, &[]).unwrap();
        assert!(!cleared.contains(START));
        assert!(cleared.contains("10.0.0.5 printer.local"));
        assert!(in_sync(&cleared, &[]).unwrap());
    }

    #[test]
    fn ipv4_ipv6_and_both_are_valid_lines() {
        assert!(valid_pin_line("10.10.10.122 test.rumenx.com"));
        assert!(valid_pin_line("2001:db8::10 app.example.com"));
        assert!(!valid_pin_line("10.10.10.122"));
        assert!(!valid_pin_line("not-an-ip example.com"));
        let both = render(
            FIXTURE,
            &[
                "203.0.113.10 app.example.com".to_string(),
                "2001:db8::10 app.example.com".to_string(),
            ],
        )
        .unwrap();
        assert!(both.contains("203.0.113.10 app.example.com"));
        assert!(both.contains("2001:db8::10 app.example.com"));
    }

    #[test]
    fn render_does_not_touch_the_real_hosts_file() {
        let hosts = Path::new("/etc/hosts");
        let before = fs::read(hosts).unwrap();
        let _ = render(FIXTURE, &["10.10.10.122 test.example.com".to_string()]);
        assert_eq!(fs::read(hosts).unwrap(), before);
    }
}

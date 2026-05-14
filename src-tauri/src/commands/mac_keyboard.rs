#[cfg(target_os = "macos")]
const G610_PORT: u16 = 19610;
#[cfg(target_os = "macos")]
const CODEX_DESKTOP_WATCHER_ENV_KEY: &str = "CODEX_DESKTOP_APPROVAL_WATCHER";
#[cfg(target_os = "macos")]
const CODEX_DESKTOP_WATCHER_MAX_SECONDS: u64 = 180;
#[cfg(target_os = "macos")]
const CLAUDE_REQUEST_HOOK_START_COMMAND: &str =
    "/usr/bin/printf 'start 180' | /usr/bin/nc 127.0.0.1 19610";
#[cfg(target_os = "macos")]
const CLAUDE_REQUEST_HOOK_STOP_COMMAND: &str = "/usr/bin/printf stop | /usr/bin/nc 127.0.0.1 19610";

use serde::Serialize;
#[cfg(target_os = "macos")]
use serde_json::{json, Map, Value};

#[cfg(target_os = "macos")]
use once_cell::sync::Lazy;
#[cfg(target_os = "macos")]
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::io::{Read, Seek, SeekFrom};
#[cfg(target_os = "macos")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Mutex,
};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant, SystemTime};

#[cfg(target_os = "macos")]
static CODEX_DESKTOP_WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static CODEX_DESKTOP_WATCHER_STOP: Lazy<Mutex<Option<mpsc::Sender<()>>>> =
    Lazy::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacKeyboardServiceState {
    pub installed: bool,
    pub running: bool,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacKeyboardDeviceStatus {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacKeyboardServicesStatus {
    pub supported: bool,
    pub g610_listening: MacKeyboardServiceState,
    pub g610_blinking: MacKeyboardServiceState,
    pub codex_desktop_watcher: MacKeyboardServiceState,
    pub claude_request_hooks: MacKeyboardServiceState,
    pub input_mapping: MacKeyboardServiceState,
    pub g610_write_mode: String,
    pub keyboard_write_mode: String,
    pub g610_led_command: Option<String>,
    pub g610_led_available: bool,
    pub apple_kbd_command: Option<String>,
    pub apple_kbd_available: bool,
    pub keyboard_devices: Vec<MacKeyboardDeviceStatus>,
    pub default_brightness: u8,
    pub blink_brightness: u8,
    pub frequency_hz: f64,
    pub burst_seconds: f64,
    pub pause_seconds: f64,
}

#[cfg(target_os = "macos")]
fn home_bin(name: &str) -> std::path::PathBuf {
    crate::config::get_home_dir().join("bin").join(name)
}

#[cfg(target_os = "macos")]
fn script_installed(name: &str) -> bool {
    home_bin(name).is_file()
}

#[cfg(target_os = "macos")]
fn run_script(name: &str) -> Result<String, String> {
    let path = home_bin(name);
    if !path.is_file() {
        return Err(format!("Missing script: {}", path.display()));
    }

    let output = std::process::Command::new(&path)
        .output()
        .map_err(|e| format!("Failed to run {}: {e}", path.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(if stderr.is_empty() {
            format!("{} exited with {}", path.display(), output.status)
        } else {
            stderr
        })
    }
}

#[cfg(target_os = "macos")]
fn run_command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {program}: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else if stderr.is_empty() {
        Err(format!("{program} exited with {}", output.status))
    } else {
        Err(stderr)
    }
}

#[cfg(target_os = "macos")]
fn kickstart_g610_launchdaemon() -> Result<String, String> {
    run_command(
        "launchctl",
        &["kickstart", "-k", "system/com.codex.g610.blink-server"],
    )
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn terminal_nc_command(command: &str) -> String {
    format!(
        "printf %s {} | nc 127.0.0.1 {}",
        shell_quote(command),
        G610_PORT
    )
}

#[cfg(target_os = "macos")]
fn open_terminal_command(command: &str, purpose: &str) -> Result<(), String> {
    let script = format!(
        r#"tell application "Terminal"
    activate
    do script "{}"
end tell"#,
        escape_applescript_string(&command)
    );

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Failed to open Terminal for {purpose}: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("osascript exited with {}", output.status)
        } else {
            stderr
        })
    }
}

#[cfg(target_os = "macos")]
fn open_terminal_for_g610_start(after_start: Option<&str>) -> Result<(), String> {
    let start_script = home_bin("codex-g610-server-start");
    if !start_script.is_file() {
        return Err(format!("Missing script: {}", start_script.display()));
    }

    let mut command = shell_quote(&start_script.to_string_lossy());
    if let Some(after_start) = after_start {
        command.push_str(" && ");
        command.push_str(after_start);
    }
    command.push_str(
        "; echo; echo 'G610 server command finished. You can close this Terminal window.'",
    );
    open_terminal_command(&command, "sudo password")
}

#[cfg(target_os = "macos")]
fn open_terminal_for_input_mapping_start() -> Result<(), String> {
    let start_script = home_bin("codex-mac-input-start");
    if !start_script.is_file() {
        return Err(format!("Missing script: {}", start_script.display()));
    }

    let command = format!(
        "echo {}; echo {}; open {} || true; open {} || true; {}; echo; echo {}",
        shell_quote(
            "Input mapper needs macOS Accessibility and Input Monitoring permissions."
        ),
        shell_quote(
            "Enable Terminal and Python if macOS blocks the event tap, then run this command again."
        ),
        shell_quote("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"),
        shell_quote("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"),
        shell_quote(&start_script.to_string_lossy()),
        shell_quote("Input mapper command finished. You can close this Terminal window.")
    );
    open_terminal_command(&command, "input mapping permissions")
}

#[cfg(target_os = "macos")]
fn ensure_g610_server_started(after_start: Option<&str>) -> Result<bool, String> {
    if let Ok(output) = kickstart_g610_launchdaemon() {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if get_g610_network_state().0 {
            return Ok(true);
        }
        if !output.is_empty() {
            eprintln!("G610 launchd kickstart returned without listener: {output}");
        }
    }

    match run_script("codex-g610-server-start") {
        Ok(_) => Ok(true),
        Err(start_error) => {
            open_terminal_for_g610_start(after_start).map_err(|terminal_error| {
                format!(
                    "{start_error}; also failed to open Terminal for sudo password: \
                     {terminal_error}"
                )
            })?;
            Ok(false)
        }
    }
}

#[cfg(target_os = "macos")]
fn send_g610_command(command: &str) -> Result<String, String> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr = SocketAddr::from(([127, 0, 0, 1], G610_PORT));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(700))
        .map_err(|e| format!("G610 server is not listening on 127.0.0.1:{G610_PORT}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(700)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(command.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.shutdown(std::net::Shutdown::Write).ok();

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    let response = response.trim().to_string();
    if response.starts_with("error") {
        return Err(response);
    }
    Ok(response)
}

#[cfg(target_os = "macos")]
fn get_g610_network_state() -> (bool, bool, Option<String>) {
    match send_g610_command("status") {
        Ok(response) => {
            let blinking = response.contains("blinking");
            (true, blinking, Some(response))
        }
        Err(error) => (false, false, Some(error)),
    }
}

#[cfg(target_os = "macos")]
fn g610_env_path() -> std::path::PathBuf {
    crate::config::get_home_dir()
        .join(".codex-g610")
        .join("g610.env")
}

#[cfg(target_os = "macos")]
fn g610_default_led_command() -> std::path::PathBuf {
    crate::config::get_home_dir()
        .join(".codex-g610")
        .join("bin")
        .join("g610-led")
}

#[cfg(target_os = "macos")]
fn apple_kbd_led_command() -> std::path::PathBuf {
    crate::config::get_home_dir()
        .join(".codex-g610")
        .join("bin")
        .join("apple-kbd-led")
}

#[cfg(target_os = "macos")]
fn preferred_python() -> String {
    let home = crate::config::get_home_dir();
    for candidate in [
        home.join("miniforge3").join("bin").join("python"),
        home.join("miniconda3").join("bin").join("python"),
        home.join("anaconda3").join("bin").join("python"),
        std::path::PathBuf::from("/usr/bin/python3"),
    ] {
        if candidate.is_file() {
            return candidate.to_string_lossy().to_string();
        }
    }
    "/usr/bin/env python3".to_string()
}

#[cfg(target_os = "macos")]
fn g610_command_available(command: &str) -> bool {
    let command = command.trim();
    if command.is_empty() {
        return false;
    }
    if command.contains('/') {
        return std::path::Path::new(command).is_file();
    }
    std::process::Command::new("sh")
        .args([
            "-lc",
            &format!("command -v {} >/dev/null 2>&1", shell_quote(command)),
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn command_status(command: &str, args: &[&str]) -> (bool, Option<String>) {
    let command = command.trim();
    if command.is_empty() {
        return (false, Some("command is empty".to_string()));
    }
    let output = std::process::Command::new(command).args(args).output();
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stdout.is_empty() { stderr } else { stdout };
            (output.status.success(), Some(detail))
        }
        Err(error) => (false, Some(error.to_string())),
    }
}

#[cfg(target_os = "macos")]
fn apple_kbd_available(command: &str) -> (bool, Option<String>) {
    command_status(command, &["--status"])
}

#[cfg(target_os = "macos")]
fn read_g610_env() -> std::collections::HashMap<String, String> {
    let path = g610_env_path();
    let Ok(content) = std::fs::read_to_string(path) else {
        return std::collections::HashMap::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn write_g610_env_value(key: &str, value: impl ToString) -> Result<(), String> {
    write_g610_env_values(&[(key, value.to_string())])
}

#[cfg(target_os = "macos")]
fn write_g610_env_values(values: &[(&str, String)]) -> Result<(), String> {
    let path = g610_env_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut env = read_g610_env();
    for (key, value) in values {
        env.insert((*key).to_string(), value.clone());
    }

    let ordered_keys = [
        "G610_DEFAULT_BRIGHTNESS",
        "G610_BLINK_BRIGHTNESS",
        "G610_FREQUENCY_HZ",
        "G610_BURST_SECONDS",
        "G610_PAUSE_SECONDS",
        "G610_WRITE_MODE",
        "G610_LED_COMMAND",
        "APPLE_KBD_COMMAND",
        CODEX_DESKTOP_WATCHER_ENV_KEY,
    ];
    let mut output = String::new();
    for key in ordered_keys {
        if let Some(value) = env.get(key) {
            output.push_str(key);
            output.push('=');
            output.push_str(value);
            output.push('\n');
        }
    }
    for (key, value) in env {
        if !ordered_keys.contains(&key.as_str()) {
            output.push_str(&key);
            output.push('=');
            output.push_str(&value);
            output.push('\n');
        }
    }

    match std::fs::write(&path, &output) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let tmp_path = path.with_extension("env.tmp");
            std::fs::write(&tmp_path, output).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn apple_kbd_helper_script(python: &str) -> String {
    format!(
        r#"#!{python}
import sys

CORE_BRIGHTNESS = "/System/Library/PrivateFrameworks/CoreBrightness.framework"


def fail(message, code=1):
    print(message, file=sys.stderr)
    raise SystemExit(code)


def load_client():
    try:
        from Foundation import NSBundle, NSClassFromString
    except Exception as error:
        fail(f"PyObjC Foundation is unavailable: {{error}}", 3)
    bundle = NSBundle.bundleWithPath_(CORE_BRIGHTNESS)
    if bundle is None or not bundle.load():
        fail("CoreBrightness.framework could not be loaded", 4)
    cls = NSClassFromString("KeyboardBrightnessClient")
    if cls is None:
        fail("KeyboardBrightnessClient is unavailable", 5)
    client = cls.alloc().init()
    ids = client.copyKeyboardBacklightIDs()
    if not ids:
        fail("No Apple keyboard backlight IDs found", 2)
    return client, list(ids)


def parse_level(value):
    value = value.strip()
    try:
        if value.lower().startswith("0x"):
            raw = int(value, 16)
            return max(0.0, min(1.0, raw / 255.0))
        if len(value) <= 2 and all(c in "0123456789abcdefABCDEF" for c in value):
            raw = int(value, 16)
            return max(0.0, min(1.0, raw / 255.0))
        raw = float(value)
    except Exception:
        fail(f"Invalid brightness value: {{value}}", 6)
    if raw > 1.0:
        raw = raw / 100.0
    return max(0.0, min(1.0, raw))


def set_brightness(client, keyboard_ids, level):
    errors = []
    for keyboard_id in keyboard_ids:
        try:
            client.setBrightness_fadeSpeed_commit_forKeyboard_(level, 0.0, True, keyboard_id)
            continue
        except Exception as error:
            errors.append(str(error))
        try:
            client.setBrightness_forKeyboard_(level, keyboard_id)
            continue
        except Exception as error:
            errors.append(str(error))
    if errors and len(errors) >= len(keyboard_ids) * 2:
        fail(errors[-1], 7)


def read_brightness(client, keyboard_id):
    try:
        return float(client.brightnessForKeyboard_(keyboard_id))
    except Exception:
        return -1.0


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "--status":
        client, keyboard_ids = load_client()
        level = read_brightness(client, keyboard_ids[0])
        print(f"ok apple-kbd ids={{len(keyboard_ids)}} brightness={{level:.3f}}")
        return 0
    if len(sys.argv) == 3 and sys.argv[1] == "-a":
        client, keyboard_ids = load_client()
        set_brightness(client, keyboard_ids, parse_level(sys.argv[2]))
        return 0
    if len(sys.argv) == 2 and sys.argv[1] == "--test":
        import time
        client, keyboard_ids = load_client()
        original = read_brightness(client, keyboard_ids[0])
        set_brightness(client, keyboard_ids, 1.0)
        time.sleep(0.4)
        set_brightness(client, keyboard_ids, 0.0)
        time.sleep(0.4)
        if original >= 0:
            set_brightness(client, keyboard_ids, original)
        return 0
    fail("usage: apple-kbd-led --status | -a <hex-or-percent>", 64)


if __name__ == "__main__":
    raise SystemExit(main())
"#
    )
}

#[cfg(target_os = "macos")]
fn ensure_apple_kbd_helper() -> Result<String, String> {
    let path = apple_kbd_led_command();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let script = apple_kbd_helper_script(&preferred_python());
    let should_write = std::fs::read_to_string(&path)
        .map(|current| current != script)
        .unwrap_or(true);
    if should_write {
        std::fs::write(&path, script).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).map_err(|error| error.to_string())?;
        }
    }
    Ok(path.to_string_lossy().to_string())
}

#[cfg(target_os = "macos")]
fn normalize_write_mode(value: Option<&String>) -> String {
    match value.map(|value| value.trim().to_lowercase()) {
        Some(value) if matches!(value.as_str(), "auto" | "g610-led" | "apple-kbd") => value,
        Some(value)
            if matches!(
                value.as_str(),
                "host-native" | "effect" | "native-all" | "native" | "direct" | "onboard"
            ) =>
        {
            value
        }
        _ => "auto".to_string(),
    }
}

#[cfg(target_os = "macos")]
fn ensure_keyboard_control_env() -> Result<(), String> {
    let env = read_g610_env();
    let existing_command = env
        .get("G610_LED_COMMAND")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let default_command = g610_default_led_command().to_string_lossy().to_string();
    let command = match existing_command {
        Some(value) if g610_command_available(value) => value.to_string(),
        _ => default_command,
    };
    let apple_command = env
        .get("APPLE_KBD_COMMAND")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(ensure_apple_kbd_helper()?);
    let mode = normalize_write_mode(env.get("G610_WRITE_MODE"));

    write_g610_env_values(&[
        ("G610_WRITE_MODE", mode),
        ("G610_LED_COMMAND", command),
        ("APPLE_KBD_COMMAND", apple_command),
    ])
}

#[cfg(target_os = "macos")]
fn clamp_brightness(value: u8) -> u8 {
    value.min(100)
}

#[cfg(target_os = "macos")]
fn clamp_frequency(value: f64) -> f64 {
    value.clamp(0.5, 10.0)
}

#[cfg(target_os = "macos")]
fn clamp_burst_seconds(value: f64) -> f64 {
    value.clamp(1.0, 60.0)
}

#[cfg(target_os = "macos")]
fn clamp_pause_seconds(value: f64) -> f64 {
    value.clamp(0.0, 120.0)
}

#[cfg(target_os = "macos")]
fn find_status_value<'a>(detail: &'a str, keys: &[&str]) -> Option<&'a str> {
    detail.split_whitespace().find_map(|part| {
        keys.iter().find_map(|key| {
            part.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix('='))
        })
    })
}

#[cfg(target_os = "macos")]
fn parse_brightness(detail: Option<&str>, keys: &[&str], fallback: u8) -> u8 {
    let Some(detail) = detail else {
        return fallback;
    };
    find_status_value(detail, keys)
        .and_then(|value| value.parse::<u8>().ok())
        .map(clamp_brightness)
        .unwrap_or(fallback)
}

#[cfg(target_os = "macos")]
fn parse_env_brightness(key: &str, fallback: u8) -> u8 {
    read_g610_env()
        .get(key)
        .and_then(|value| value.parse::<u8>().ok())
        .map(clamp_brightness)
        .unwrap_or(fallback)
}

#[cfg(target_os = "macos")]
fn parse_number(detail: Option<&str>, keys: &[&str], fallback: f64, clamp: fn(f64) -> f64) -> f64 {
    let Some(detail) = detail else {
        return fallback;
    };
    find_status_value(detail, keys)
        .and_then(|value| value.parse::<f64>().ok())
        .map(clamp)
        .unwrap_or(fallback)
}

#[cfg(target_os = "macos")]
fn parse_env_number(key: &str, fallback: f64, clamp: fn(f64) -> f64) -> f64 {
    read_g610_env()
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .map(clamp)
        .unwrap_or(fallback)
}

#[cfg(target_os = "macos")]
fn input_mapping_state() -> MacKeyboardServiceState {
    let installed = script_installed("codex-mac-input-status");
    if !installed {
        return MacKeyboardServiceState {
            installed,
            running: false,
            status: "missing".to_string(),
            detail: Some("~/bin/codex-mac-input-status was not found".to_string()),
        };
    }

    match run_script("codex-mac-input-status") {
        Ok(output) => MacKeyboardServiceState {
            installed,
            running: output.contains("running"),
            status: if output.contains("running") {
                "running".to_string()
            } else {
                "stopped".to_string()
            },
            detail: Some(output),
        },
        Err(error) => MacKeyboardServiceState {
            installed,
            running: false,
            status: "error".to_string(),
            detail: Some(error),
        },
    }
}

#[cfg(target_os = "macos")]
fn claude_request_hook_entry(command: &str) -> Value {
    json!({
        "matcher": "",
        "hooks": [
            {
                "type": "command",
                "command": command
            }
        ]
    })
}

#[cfg(target_os = "macos")]
fn is_claude_request_hook_entry(value: &Value, command: &str) -> bool {
    value
        .get("hooks")
        .and_then(Value::as_array)
        .map(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("type").and_then(Value::as_str) == Some("command")
                    && hook.get("command").and_then(Value::as_str) == Some(command)
            })
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn claude_settings_path() -> std::path::PathBuf {
    crate::config::get_claude_settings_path()
}

#[cfg(target_os = "macos")]
fn read_claude_settings_value() -> Result<Value, String> {
    let path = claude_settings_path();
    if !path.exists() {
        return Ok(json!({}));
    }
    let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    if content.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&content).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(target_os = "macos")]
fn write_claude_settings_value(value: &Value) -> Result<(), String> {
    let path = claude_settings_path();
    crate::config::write_json_file(&path, value).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn ensure_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

#[cfg(target_os = "macos")]
fn hook_event_entries_mut<'a>(root: &'a mut Map<String, Value>, event: &str) -> &'a mut Vec<Value> {
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks_obj = hooks.as_object_mut().expect("hooks object was just set");
    let entries = hooks_obj.entry(event).or_insert_with(|| json!([]));
    if !entries.is_array() {
        *entries = json!([]);
    }
    entries.as_array_mut().expect("entries array was just set")
}

#[cfg(target_os = "macos")]
fn add_claude_hook_if_missing(root: &mut Map<String, Value>, event: &str, command: &str) -> bool {
    let entries = hook_event_entries_mut(root, event);
    if entries
        .iter()
        .any(|entry| is_claude_request_hook_entry(entry, command))
    {
        return false;
    }
    entries.push(claude_request_hook_entry(command));
    true
}

#[cfg(target_os = "macos")]
fn remove_claude_hook(root: &mut Map<String, Value>, event: &str, command: &str) -> bool {
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    let Some(entries) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
        return false;
    };
    let before = entries.len();
    entries.retain(|entry| !is_claude_request_hook_entry(entry, command));
    entries.len() != before
}

#[cfg(target_os = "macos")]
fn set_claude_request_hooks_enabled(enabled: bool) -> Result<(), String> {
    let mut root = ensure_object(read_claude_settings_value()?);
    if enabled {
        add_claude_hook_if_missing(
            &mut root,
            "PermissionRequest",
            CLAUDE_REQUEST_HOOK_START_COMMAND,
        );
        add_claude_hook_if_missing(&mut root, "PostToolUse", CLAUDE_REQUEST_HOOK_STOP_COMMAND);
        add_claude_hook_if_missing(&mut root, "Stop", CLAUDE_REQUEST_HOOK_STOP_COMMAND);
    } else {
        remove_claude_hook(
            &mut root,
            "PermissionRequest",
            CLAUDE_REQUEST_HOOK_START_COMMAND,
        );
        remove_claude_hook(&mut root, "PostToolUse", CLAUDE_REQUEST_HOOK_STOP_COMMAND);
        remove_claude_hook(&mut root, "Stop", CLAUDE_REQUEST_HOOK_STOP_COMMAND);
    }
    write_claude_settings_value(&Value::Object(root))
}

#[cfg(target_os = "macos")]
fn claude_request_hooks_enabled() -> bool {
    let Ok(Value::Object(root)) = read_claude_settings_value() else {
        return false;
    };
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    let has_start = hooks
        .get("PermissionRequest")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .any(|entry| is_claude_request_hook_entry(entry, CLAUDE_REQUEST_HOOK_START_COMMAND))
        })
        .unwrap_or(false);
    let has_post_stop = hooks
        .get("PostToolUse")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .any(|entry| is_claude_request_hook_entry(entry, CLAUDE_REQUEST_HOOK_STOP_COMMAND))
        })
        .unwrap_or(false);
    let has_stop = hooks
        .get("Stop")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .any(|entry| is_claude_request_hook_entry(entry, CLAUDE_REQUEST_HOOK_STOP_COMMAND))
        })
        .unwrap_or(false);
    has_start && has_post_stop && has_stop
}

#[cfg(target_os = "macos")]
fn claude_request_hooks_state() -> MacKeyboardServiceState {
    let path = claude_settings_path();
    let supported = path
        .parent()
        .map(|parent| parent.exists() || std::fs::create_dir_all(parent).is_ok())
        .unwrap_or(false);
    let running = claude_request_hooks_enabled();
    MacKeyboardServiceState {
        installed: supported,
        running,
        status: if running { "installed" } else { "stopped" }.to_string(),
        detail: Some(path.display().to_string()),
    }
}

#[cfg(target_os = "macos")]
fn codex_desktop_watcher_enabled() -> bool {
    read_g610_env()
        .get(CODEX_DESKTOP_WATCHER_ENV_KEY)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn codex_desktop_log_root() -> std::path::PathBuf {
    crate::config::get_home_dir()
        .join("Library")
        .join("Logs")
        .join("com.openai.codex")
}

#[cfg(target_os = "macos")]
fn newest_codex_desktop_log() -> Option<std::path::PathBuf> {
    fn visit_dir(dir: &std::path::Path, best: &mut Option<(SystemTime, std::path::PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                visit_dir(&path, best);
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !(name.starts_with("codex-desktop-")
                && name.contains("-t0-")
                && name.ends_with(".log"))
            {
                continue;
            }
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if best
                .as_ref()
                .map(|(best_time, _)| modified > *best_time)
                .unwrap_or(true)
            {
                *best = Some((modified, path));
            }
        }
    }

    let mut best = None;
    visit_dir(&codex_desktop_log_root(), &mut best);
    best.map(|(_, path)| path)
}

#[cfg(target_os = "macos")]
fn parse_show_approval_request_id(line: &str) -> Option<String> {
    if !line.contains("[desktop-notifications] show approval") {
        return None;
    }
    line.split_whitespace()
        .find_map(|part| part.strip_prefix("requestId=").map(ToString::to_string))
}

#[cfg(target_os = "macos")]
fn parse_approval_response_id(line: &str) -> Option<String> {
    if !(line.contains("Sending server response")
        && line.contains("requestApproval")
        && line.contains("response="))
    {
        return None;
    }
    line.split_whitespace()
        .find_map(|part| part.strip_prefix("id=").map(ToString::to_string))
}

#[cfg(target_os = "macos")]
fn process_codex_desktop_log_line(
    line: &str,
    pending: &mut HashSet<String>,
    last_start: &mut Option<Instant>,
) {
    if let Some(request_id) = parse_show_approval_request_id(line) {
        let was_empty = pending.is_empty();
        pending.insert(request_id);
        if was_empty || last_start.is_none() {
            let _ = send_g610_command(&format!("start {CODEX_DESKTOP_WATCHER_MAX_SECONDS}"));
            *last_start = Some(Instant::now());
        }
        return;
    }

    if let Some(request_id) = parse_approval_response_id(line) {
        pending.remove(&request_id);
        if pending.is_empty() {
            let _ = send_g610_command("stop");
            *last_start = None;
        }
    }
}

#[cfg(target_os = "macos")]
fn follow_codex_desktop_log(stop_rx: mpsc::Receiver<()>) {
    let mut current_log: Option<std::path::PathBuf> = None;
    let mut offset = 0_u64;
    let mut pending = HashSet::new();
    let mut last_start: Option<Instant> = None;

    loop {
        if stop_rx.try_recv().is_ok() {
            let _ = send_g610_command("stop");
            break;
        }

        if last_start
            .map(|started| {
                started.elapsed() >= Duration::from_secs(CODEX_DESKTOP_WATCHER_MAX_SECONDS)
            })
            .unwrap_or(false)
        {
            pending.clear();
            let _ = send_g610_command("stop");
            last_start = None;
        }

        let newest = newest_codex_desktop_log();
        if newest != current_log {
            current_log = newest;
            offset = current_log
                .as_ref()
                .and_then(|path| std::fs::metadata(path).ok())
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            pending.clear();
            last_start = None;
        }

        if let Some(path) = current_log.as_ref() {
            if let Ok(mut file) = std::fs::File::open(path) {
                let len = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
                if len < offset {
                    offset = 0;
                }
                if len > offset && file.seek(SeekFrom::Start(offset)).is_ok() {
                    let mut data = String::new();
                    if file.read_to_string(&mut data).is_ok() {
                        offset = len;
                        for line in data.lines() {
                            process_codex_desktop_log_line(line, &mut pending, &mut last_start);
                        }
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }
    CODEX_DESKTOP_WATCHER_RUNNING.store(false, Ordering::SeqCst);
}

#[cfg(target_os = "macos")]
fn start_codex_desktop_watcher_runtime() -> Result<(), String> {
    if CODEX_DESKTOP_WATCHER_RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel();
    {
        let mut stop_slot = CODEX_DESKTOP_WATCHER_STOP
            .lock()
            .map_err(|_| "Codex Desktop watcher lock poisoned".to_string())?;
        *stop_slot = Some(tx);
    }

    std::thread::Builder::new()
        .name("codex-desktop-approval-watcher".to_string())
        .spawn(move || follow_codex_desktop_log(rx))
        .map_err(|error| {
            CODEX_DESKTOP_WATCHER_RUNNING.store(false, Ordering::SeqCst);
            format!("Failed to start Codex Desktop watcher: {error}")
        })?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn stop_codex_desktop_watcher_runtime() {
    let was_running = CODEX_DESKTOP_WATCHER_RUNNING.load(Ordering::SeqCst);
    let mut requested_stop = false;
    if let Ok(mut stop_slot) = CODEX_DESKTOP_WATCHER_STOP.lock() {
        if let Some(tx) = stop_slot.take() {
            let _ = tx.send(());
            requested_stop = true;
        }
    }
    if was_running || requested_stop {
        let _ = send_g610_command("stop");
    }
}

#[cfg(target_os = "macos")]
fn codex_desktop_watcher_state() -> MacKeyboardServiceState {
    let log_root = codex_desktop_log_root();
    let installed = log_root.is_dir();
    let enabled = codex_desktop_watcher_enabled();
    let running = CODEX_DESKTOP_WATCHER_RUNNING.load(Ordering::SeqCst);
    let detail = newest_codex_desktop_log()
        .map(|path| path.display().to_string())
        .or_else(|| {
            Some(format!(
                "No Codex Desktop t0 log under {}",
                log_root.display()
            ))
        });

    MacKeyboardServiceState {
        installed,
        running,
        status: if running {
            "watching".to_string()
        } else if enabled {
            "enabled".to_string()
        } else {
            "stopped".to_string()
        },
        detail,
    }
}

#[cfg(target_os = "macos")]
fn keyboard_write_mode_for_ui(mode: &str) -> String {
    if matches!(mode, "auto" | "g610-led" | "apple-kbd") {
        mode.to_string()
    } else {
        "g610-led".to_string()
    }
}

#[cfg(target_os = "macos")]
fn detected_keyboard_devices(
    g610_led_command: Option<&str>,
    g610_led_available: bool,
    apple_kbd_command: Option<&str>,
    apple_kbd_available: bool,
    apple_detail: Option<String>,
) -> Vec<MacKeyboardDeviceStatus> {
    vec![
        MacKeyboardDeviceStatus {
            id: "g610-led".to_string(),
            label: "Logitech G610 / g810-led compatible".to_string(),
            available: g610_led_available,
            detail: g610_led_command.map(ToString::to_string),
        },
        MacKeyboardDeviceStatus {
            id: "apple-kbd".to_string(),
            label: "Apple Keyboard Backlight".to_string(),
            available: apple_kbd_available,
            detail: apple_detail.or_else(|| apple_kbd_command.map(ToString::to_string)),
        },
    ]
}

#[cfg(target_os = "macos")]
pub fn restore_codex_desktop_watcher_on_launch() {
    if !codex_desktop_watcher_enabled() {
        return;
    }
    if let Err(error) = ensure_keyboard_control_env()
        .and_then(|_| ensure_g610_server_started(None).map(|_| ()))
        .and_then(|_| start_codex_desktop_watcher_runtime())
    {
        log::warn!("Failed to restore Codex Desktop approval watcher: {error}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn restore_codex_desktop_watcher_on_launch() {}

pub fn stop_codex_desktop_watcher_on_exit() {
    #[cfg(target_os = "macos")]
    stop_codex_desktop_watcher_runtime();
}

#[cfg(target_os = "macos")]
fn status_impl() -> MacKeyboardServicesStatus {
    if let Err(error) = ensure_keyboard_control_env() {
        eprintln!("Failed to ensure keyboard control env: {error}");
    }
    let start_installed = script_installed("codex-g610-server-start");
    let stop_installed = script_installed("codex-g610-server-stop");
    let status_installed = script_installed("codex-g610-server-status");
    let g610_installed = start_installed && stop_installed && status_installed;
    let (listening, blinking, g610_detail) = get_g610_network_state();
    let default_brightness = parse_brightness(
        g610_detail.as_deref(),
        &["default", "default-brightness", "default_brightness"],
        parse_env_brightness("G610_DEFAULT_BRIGHTNESS", 0),
    );
    let blink_brightness = parse_brightness(
        g610_detail.as_deref(),
        &["blink", "blink-brightness", "blink_brightness"],
        parse_env_brightness("G610_BLINK_BRIGHTNESS", 100),
    );
    let frequency_hz = parse_number(
        g610_detail.as_deref(),
        &["frequency", "frequency-hz", "frequency_hz"],
        parse_env_number("G610_FREQUENCY_HZ", 3.0, clamp_frequency),
        clamp_frequency,
    );
    let burst_seconds = parse_number(
        g610_detail.as_deref(),
        &["burst", "burst-seconds", "burst_seconds"],
        parse_env_number("G610_BURST_SECONDS", 5.0, clamp_burst_seconds),
        clamp_burst_seconds,
    );
    let pause_seconds = parse_number(
        g610_detail.as_deref(),
        &["pause", "pause-seconds", "pause_seconds"],
        parse_env_number("G610_PAUSE_SECONDS", 15.0, clamp_pause_seconds),
        clamp_pause_seconds,
    );
    let env = read_g610_env();
    let g610_write_mode = env
        .get("G610_WRITE_MODE")
        .cloned()
        .unwrap_or_else(|| "g610-led".to_string());
    let g610_led_command = env.get("G610_LED_COMMAND").cloned();
    let g610_led_available = g610_led_command
        .as_deref()
        .map(g610_command_available)
        .unwrap_or(false);
    let apple_kbd_command = env.get("APPLE_KBD_COMMAND").cloned();
    let (apple_kbd_available, apple_detail) = apple_kbd_command
        .as_deref()
        .map(apple_kbd_available)
        .unwrap_or((false, None));
    let keyboard_write_mode = keyboard_write_mode_for_ui(&g610_write_mode);
    let keyboard_devices = detected_keyboard_devices(
        g610_led_command.as_deref(),
        g610_led_available,
        apple_kbd_command.as_deref(),
        apple_kbd_available,
        apple_detail,
    );

    MacKeyboardServicesStatus {
        supported: true,
        g610_listening: MacKeyboardServiceState {
            installed: g610_installed,
            running: listening,
            status: if listening { "listening" } else { "stopped" }.to_string(),
            detail: g610_detail.clone(),
        },
        g610_blinking: MacKeyboardServiceState {
            installed: g610_installed,
            running: blinking,
            status: if blinking { "blinking" } else { "idle" }.to_string(),
            detail: g610_detail,
        },
        codex_desktop_watcher: codex_desktop_watcher_state(),
        claude_request_hooks: claude_request_hooks_state(),
        input_mapping: input_mapping_state(),
        g610_write_mode,
        keyboard_write_mode,
        g610_led_command,
        g610_led_available,
        apple_kbd_command,
        apple_kbd_available,
        keyboard_devices,
        default_brightness,
        blink_brightness,
        frequency_hz,
        burst_seconds,
        pause_seconds,
    }
}

#[cfg(not(target_os = "macos"))]
fn unsupported_state() -> MacKeyboardServiceState {
    MacKeyboardServiceState {
        installed: false,
        running: false,
        status: "unsupported".to_string(),
        detail: Some("Mac keyboard controls are only available on macOS".to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn status_impl() -> MacKeyboardServicesStatus {
    MacKeyboardServicesStatus {
        supported: false,
        g610_listening: unsupported_state(),
        g610_blinking: unsupported_state(),
        codex_desktop_watcher: unsupported_state(),
        claude_request_hooks: unsupported_state(),
        input_mapping: unsupported_state(),
        g610_write_mode: "unsupported".to_string(),
        keyboard_write_mode: "unsupported".to_string(),
        g610_led_command: None,
        g610_led_available: false,
        apple_kbd_command: None,
        apple_kbd_available: false,
        keyboard_devices: Vec::new(),
        default_brightness: 0,
        blink_brightness: 100,
        frequency_hz: 3.0,
        burst_seconds: 5.0,
        pause_seconds: 15.0,
    }
}

#[tauri::command]
pub async fn get_mac_keyboard_services_status() -> Result<MacKeyboardServicesStatus, String> {
    Ok(status_impl())
}

#[tauri::command]
pub async fn set_mac_g610_listening(enabled: bool) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        ensure_keyboard_control_env()?;
        if enabled {
            ensure_g610_server_started(None)?;
        } else {
            run_script("codex-g610-server-stop")?;
        }
        return Ok(status_impl());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_g610_blinking(enabled: bool) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        ensure_keyboard_control_env()?;
        let (listening, _, _) = get_g610_network_state();
        if !listening {
            let command = if enabled { "start" } else { "stop" };
            let after_start = terminal_nc_command(command);
            if !ensure_g610_server_started(Some(&after_start))? {
                return Ok(status_impl());
            }
        }
        send_g610_command(if enabled { "start" } else { "stop" })?;
        return Ok(status_impl());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_keyboard_write_mode(
    mode: String,
) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        ensure_keyboard_control_env()?;
        let normalized = match mode.trim().to_lowercase().as_str() {
            "auto" => "auto",
            "g610-led" => "g610-led",
            "apple-kbd" => "apple-kbd",
            _ => return Err(format!("Unknown keyboard write mode: {mode}")),
        };
        write_g610_env_value("G610_WRITE_MODE", normalized)?;
        let (listening, _, _) = get_g610_network_state();
        if listening {
            let _ = send_g610_command(&format!("set mode {normalized}"));
        }
        return Ok(status_impl());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = mode;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn test_mac_keyboard_blink() -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        ensure_keyboard_control_env()?;
        let (listening, _, _) = get_g610_network_state();
        if !listening && !ensure_g610_server_started(Some(&terminal_nc_command("start 3")))? {
            return Ok(status_impl());
        }
        send_g610_command("start 3")?;
        return Ok(status_impl());
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_codex_desktop_watcher(
    enabled: bool,
) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        write_g610_env_value(
            CODEX_DESKTOP_WATCHER_ENV_KEY,
            if enabled { "1" } else { "0" },
        )?;
        if enabled {
            ensure_keyboard_control_env()?;
            if !ensure_g610_server_started(None)? {
                return Ok(status_impl());
            }
            start_codex_desktop_watcher_runtime()?;
        } else {
            stop_codex_desktop_watcher_runtime();
        }
        return Ok(status_impl());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_claude_request_hooks(
    enabled: bool,
) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        if enabled {
            set_claude_request_hooks_enabled(true)?;
            ensure_keyboard_control_env()?;
            if !ensure_g610_server_started(None)? {
                return Ok(status_impl());
            }
        } else {
            set_claude_request_hooks_enabled(false)?;
            let _ = send_g610_command("stop");
        }
        return Ok(status_impl());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_g610_default_brightness(
    brightness: u8,
) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let brightness = clamp_brightness(brightness);
        let command = format!("set default-brightness {brightness}");
        ensure_keyboard_control_env()?;
        write_g610_env_value("G610_DEFAULT_BRIGHTNESS", brightness)?;
        let (listening, _, _) = get_g610_network_state();
        if !listening {
            if !ensure_g610_server_started(Some(&terminal_nc_command(&command)))? {
                return Ok(status_impl());
            }
        }
        send_g610_command(&command)?;
        let mut status = status_impl();
        status.default_brightness = brightness;
        return Ok(status);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = brightness;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_g610_blink_brightness(
    brightness: u8,
) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let brightness = clamp_brightness(brightness);
        let command = format!("set blink-brightness {brightness}");
        ensure_keyboard_control_env()?;
        write_g610_env_value("G610_BLINK_BRIGHTNESS", brightness)?;
        let (listening, _, _) = get_g610_network_state();
        if !listening {
            if !ensure_g610_server_started(Some(&terminal_nc_command(&command)))? {
                return Ok(status_impl());
            }
        }
        send_g610_command(&command)?;
        let mut status = status_impl();
        status.blink_brightness = brightness;
        return Ok(status);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = brightness;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_g610_frequency(
    frequency_hz: f64,
) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let frequency_hz = clamp_frequency(frequency_hz);
        let command = format!("set frequency {frequency_hz:.1}");
        ensure_keyboard_control_env()?;
        write_g610_env_value("G610_FREQUENCY_HZ", format!("{frequency_hz:.1}"))?;
        let (listening, _, _) = get_g610_network_state();
        if !listening {
            if !ensure_g610_server_started(Some(&terminal_nc_command(&command)))? {
                return Ok(status_impl());
            }
        }
        send_g610_command(&command)?;
        let mut status = status_impl();
        status.frequency_hz = frequency_hz;
        return Ok(status);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = frequency_hz;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_g610_burst_seconds(seconds: f64) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let seconds = clamp_burst_seconds(seconds);
        let command = format!("set burst-seconds {seconds:.1}");
        ensure_keyboard_control_env()?;
        write_g610_env_value("G610_BURST_SECONDS", format!("{seconds:.1}"))?;
        let (listening, _, _) = get_g610_network_state();
        if !listening {
            if !ensure_g610_server_started(Some(&terminal_nc_command(&command)))? {
                return Ok(status_impl());
            }
        }
        send_g610_command(&command)?;
        let mut status = status_impl();
        status.burst_seconds = seconds;
        return Ok(status);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = seconds;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_g610_pause_seconds(seconds: f64) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let seconds = clamp_pause_seconds(seconds);
        let command = format!("set pause-seconds {seconds:.1}");
        ensure_keyboard_control_env()?;
        write_g610_env_value("G610_PAUSE_SECONDS", format!("{seconds:.1}"))?;
        let (listening, _, _) = get_g610_network_state();
        if !listening {
            if !ensure_g610_server_started(Some(&terminal_nc_command(&command)))? {
                return Ok(status_impl());
            }
        }
        send_g610_command(&command)?;
        let mut status = status_impl();
        status.pause_seconds = seconds;
        return Ok(status);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = seconds;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[tauri::command]
pub async fn set_mac_input_mapping(enabled: bool) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        if enabled {
            if let Err(start_error) = run_script("codex-mac-input-start") {
                open_terminal_for_input_mapping_start().map_err(|terminal_error| {
                    format!(
                        "{start_error}; also failed to open Terminal for input mapping \
                         permissions: {terminal_error}"
                    )
                })?;
                return Ok(status_impl());
            }
        } else {
            run_script("codex-mac-input-stop")?;
        }
        return Ok(status_impl());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Err("Mac keyboard controls are only available on macOS".to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_macos_status_is_supported_false() {
        #[cfg(not(target_os = "macos"))]
        {
            let status = super::status_impl();
            assert!(!status.supported);
            assert_eq!(status.g610_listening.status, "unsupported");
        }
    }
}

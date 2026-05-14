#[cfg(target_os = "macos")]
const G610_PORT: u16 = 19610;

use serde::Serialize;

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
pub struct MacKeyboardServicesStatus {
    pub supported: bool,
    pub g610_listening: MacKeyboardServiceState,
    pub g610_blinking: MacKeyboardServiceState,
    pub input_mapping: MacKeyboardServiceState,
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
    Ok(response.trim().to_string())
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
fn status_impl() -> MacKeyboardServicesStatus {
    let start_installed = script_installed("codex-g610-server-start");
    let stop_installed = script_installed("codex-g610-server-stop");
    let status_installed = script_installed("codex-g610-server-status");
    let g610_installed = start_installed && stop_installed && status_installed;
    let (listening, blinking, g610_detail) = get_g610_network_state();
    let default_brightness = parse_brightness(
        g610_detail.as_deref(),
        &["default", "default-brightness", "default_brightness"],
        0,
    );
    let blink_brightness = parse_brightness(
        g610_detail.as_deref(),
        &["blink", "blink-brightness", "blink_brightness"],
        100,
    );
    let frequency_hz = parse_number(
        g610_detail.as_deref(),
        &["frequency", "frequency-hz", "frequency_hz"],
        3.0,
        clamp_frequency,
    );
    let burst_seconds = parse_number(
        g610_detail.as_deref(),
        &["burst", "burst-seconds", "burst_seconds"],
        5.0,
        clamp_burst_seconds,
    );
    let pause_seconds = parse_number(
        g610_detail.as_deref(),
        &["pause", "pause-seconds", "pause_seconds"],
        15.0,
        clamp_pause_seconds,
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
        input_mapping: input_mapping_state(),
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
        input_mapping: unsupported_state(),
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
pub async fn set_mac_g610_default_brightness(
    brightness: u8,
) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let brightness = clamp_brightness(brightness);
        let command = format!("set default-brightness {brightness}");
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

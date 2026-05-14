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
            run_script("codex-g610-server-start")?;
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
            run_script("codex-g610-server-start")?;
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
pub async fn set_mac_input_mapping(enabled: bool) -> Result<MacKeyboardServicesStatus, String> {
    #[cfg(target_os = "macos")]
    {
        run_script(if enabled {
            "codex-mac-input-start"
        } else {
            "codex-mac-input-stop"
        })?;
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

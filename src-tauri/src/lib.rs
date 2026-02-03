use serde::Serialize;
use std::process::Command;
use tauri::{
    command, AppHandle,
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[derive(Serialize, Clone)]
pub struct PortInfo {
    pub pid: String,
    pub process: String,
    pub port: String,
    pub user: String,
    pub is_self: bool,
}

/// Collect all PIDs in our process family (self, parent, siblings, children)
fn get_family_pids() -> Vec<String> {
    let own_pid = std::process::id().to_string();
    let mut pids = vec![own_pid.clone()];

    // Get parent PID
    if let Ok(output) = Command::new("ps")
        .args(["-o", "ppid=", "-p", &own_pid])
        .output()
    {
        let ppid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !ppid.is_empty() {
            pids.push(ppid.clone());

            // Get all children of parent (our siblings, including dev server)
            if let Ok(output) = Command::new("pgrep").args(["-P", &ppid]).output() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let pid = line.trim().to_string();
                    if !pid.is_empty() {
                        pids.push(pid);
                    }
                }
            }
        }
    }

    // Also get our own children
    if let Ok(output) = Command::new("pgrep").args(["-P", &own_pid]).output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let pid = line.trim().to_string();
            if !pid.is_empty() {
                pids.push(pid);
            }
        }
    }

    pids
}

/// Get all listening TCP ports using lsof
#[command]
fn get_listening_ports() -> Result<Vec<PortInfo>, String> {
    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P"])
        .output()
        .map_err(|e| format!("Failed to execute lsof: {}. Is lsof installed?", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.is_empty() {
            // lsof returns non-zero with no stderr when there are no listening ports
            return Ok(Vec::new());
        }
        return Err(format!("lsof error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ports = Vec::new();
    let family_pids = get_family_pids();

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();

        // lsof output: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME (STATE)
        // Index:       0       1   2    3  4    5      6        7    8    9
        // NAME at index 8 contains the port (e.g., *:8080, 127.0.0.1:3000, [::1]:6379)
        if parts.len() >= 10 {
            let name = parts[8];

            if let Some(port_str) = name.rsplit(':').next() {
                if let Ok(_port) = port_str.parse::<u16>() {
                    let pid = parts[1].to_string();
                    let is_self = family_pids.contains(&pid);
                    ports.push(PortInfo {
                        process: parts[0].to_string(),
                        pid,
                        user: parts[2].to_string(),
                        port: port_str.to_string(),
                        is_self,
                    });
                }
            }
        }
    }

    // Deduplicate by port (lsof shows IPv4 and IPv6 separately)
    ports.sort_by(|a, b| {
        a.port
            .parse::<u16>()
            .unwrap_or(0)
            .cmp(&b.port.parse::<u16>().unwrap_or(0))
    });
    ports.dedup_by(|a, b| a.port == b.port);

    Ok(ports)
}

/// Get the current app's PID
#[command]
fn get_own_pid() -> u32 {
    std::process::id()
}

/// Gracefully quit the app
#[command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Kill a process by PID using SIGKILL
#[command]
fn kill_port_process(pid: String) -> Result<String, String> {
    // Validate PID is numeric to prevent command injection
    if !pid.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid PID format".to_string());
    }

    let status = Command::new("kill")
        .args(["-9", &pid])
        .status()
        .map_err(|e| format!("Failed to execute kill: {}", e))?;

    if status.success() {
        Ok(format!("Successfully killed process {}", pid))
    } else {
        Err(format!(
            "Failed to kill PID {}. Permission denied or process not found.",
            pid
        ))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Create tray menu
            let quit_item = MenuItem::with_id(app, "quit", "Quit Port Tray", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;

            // Build system tray
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Port Tray")
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    // Left click shows the window
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_listening_ports, kill_port_process, get_own_pid, quit_app])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

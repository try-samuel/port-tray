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

// ==========================================
// macOS & Linux Implementation
// ==========================================
#[cfg(not(target_os = "windows"))]
fn get_family_pids() -> Vec<String> {
    let own_pid = std::process::id().to_string();
    let mut pids = vec![own_pid.clone()];

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

#[cfg(not(target_os = "windows"))]
fn get_listening_ports_impl() -> Result<Vec<PortInfo>, String> {
    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P"])
        .output()
        .map_err(|e| format!("Failed to execute lsof: {}. Is lsof installed?", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.is_empty() {
            return Ok(Vec::new());
        }
        return Err(format!("lsof error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ports = Vec::new();
    let family_pids = get_family_pids();

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
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
    Ok(ports)
}

#[cfg(not(target_os = "windows"))]
fn kill_port_process_impl(pid: &str) -> Result<String, String> {
    let status = Command::new("kill")
        .args(["-9", pid])
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

// ==========================================
// Windows Implementation
// ==========================================
#[cfg(target_os = "windows")]
fn get_family_pids() -> Vec<String> {
    let own_pid = std::process::id().to_string();
    let mut pids = vec![own_pid.clone()];

    if let Ok(output) = Command::new("wmic")
        .args([
            "process",
            "where",
            &format!("ParentProcessId={}", own_pid),
            "get",
            "ProcessId",
        ])
        .output()
    {
        for line in String::from_utf8_lossy(&output.stdout).lines().skip(1) {
            let pid = line.trim().to_string();
            if !pid.is_empty() {
                pids.push(pid);
            }
        }
    }
    pids
}

#[cfg(target_os = "windows")]
fn build_process_map() -> std::collections::HashMap<String, (String, String)> {
    use std::collections::HashMap;
    let mut map: HashMap<String, (String, String)> = HashMap::new();

    // Call tasklist once and build a PID -> (process_name, user) map
    if let Ok(output) = Command::new("tasklist")
        .args(["/V", "/FO", "CSV", "/NH"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // CSV format: "name.exe","PID","Session","Session#","Mem","Status","User","CPU","Title"
            let fields: Vec<&str> = line.split("\",\"").collect();
            if fields.len() >= 7 {
                let process_name = fields[0].trim_matches('"').to_string();
                let pid = fields[1].trim_matches('"').to_string();
                let user = fields[6].trim_matches('"').trim().to_string();
                map.insert(pid, (process_name, user));
            }
        }
    }
    map
}

#[cfg(target_os = "windows")]
fn get_listening_ports_impl() -> Result<Vec<PortInfo>, String> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| format!("Failed to execute netstat: {}", e))?;

    if !output.status.success() {
        return Err("netstat command failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ports = Vec::new();
    let family_pids = get_family_pids();
    let process_map = build_process_map();

    for line in stdout.lines() {
        if !line.contains("LISTENING") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        // Format: TCP  0.0.0.0:8080  0.0.0.0:0  LISTENING  1234
        if parts.len() >= 5 {
            let local_addr = parts[1];
            let pid = parts[4].to_string();

            if let Some(port_str) = local_addr.rsplit(':').next() {
                if let Ok(_port) = port_str.parse::<u16>() {
                    let is_self = family_pids.contains(&pid);

                    let (process_name, user) = process_map
                        .get(&pid)
                        .cloned()
                        .unwrap_or_else(|| ("Unknown".to_string(), "Unknown".to_string()));

                    ports.push(PortInfo {
                        process: process_name,
                        pid,
                        user,
                        port: port_str.to_string(),
                        is_self,
                    });
                }
            }
        }
    }
    Ok(ports)
}

#[cfg(target_os = "windows")]
fn kill_port_process_impl(pid: &str) -> Result<String, String> {
    let status = Command::new("taskkill")
        .args(["/F", "/PID", pid])
        .status()
        .map_err(|e| format!("Failed to execute taskkill: {}", e))?;

    if status.success() {
        Ok(format!("Successfully killed process {}", pid))
    } else {
        Err(format!(
            "Failed to kill PID {}. Permission denied or process not found.",
            pid
        ))
    }
}

// ==========================================
// Tauri Commands (platform-agnostic)
// ==========================================

/// Get all listening TCP ports
#[command]
fn get_listening_ports() -> Result<Vec<PortInfo>, String> {
    let mut ports = get_listening_ports_impl()?;

    // Deduplicate by port (lsof/netstat can show IPv4 and IPv6 separately)
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

/// Kill a process by PID
#[command]
fn kill_port_process(pid: String) -> Result<String, String> {
    // Validate PID is numeric to prevent command injection
    if !pid.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid PID format".to_string());
    }
    kill_port_process_impl(&pid)
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
        .invoke_handler(tauri::generate_handler![
            get_listening_ports,
            kill_port_process,
            get_own_pid,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

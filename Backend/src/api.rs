use axum::{
    routing::{post, get},
    Router, Json, extract::State, response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{CorsLayer, Any};
use std::{net::SocketAddr, collections::HashMap, sync::Arc, path::Path, io, io::BufReader, io::BufRead, fs::{File, OpenOptions}, process};
use walkdir::WalkDir;
use std::io::Write;
use chrono::{Local, TimeZone, Datelike, Duration, NaiveDate};
use tokio::net::TcpListener;
use std::fs;
use dirs::home_dir;
use anyhow::{Result, Context};
use sysinfo::{System, SystemExt, ProcessExt, DiskExt, CpuExt};
use std::process::Command;
use std::sync::Mutex;
use lazy_static::lazy_static;

#[derive(Debug, Deserialize)]
struct CommandRequest {
    command: String,
}

#[derive(Debug, Serialize)]
struct CommandResponse {
    response: String,
}

#[derive(Debug, Serialize)]
struct CommandOutput {
    response: String,
}

#[derive(Debug, Clone, Serialize)]
struct SystemState {
    timestamp: String,
    total_memory: u64,
    used_memory: u64,
    free_memory: u64,
    cpu_usage: f32,
    disk_free: u64,
}

#[derive(Debug, Clone, Serialize)]
struct BehaviorLog {
    timestamp: String,
    command: String,
    input: String,
    output: String,
    status: String,
    duration_ms: u128,
    state_before: SystemState,
    state_after: SystemState,
}

lazy_static! {
    static ref BEHAVIOR_LOGS: Mutex<Vec<BehaviorLog>> = Mutex::new(Vec::new());
}

fn capture_system_state() -> SystemState {
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let free_memory = sys.free_memory();
    let mut cpu_usage = 0.0;
    
    for cpu in sys.cpus() {
        cpu_usage += cpu.cpu_usage();
    }
    cpu_usage /= sys.cpus().len().max(1) as f32;
    
    let disk_free = sys.disks().iter().map(|d| d.available_space()).sum();
    
    SystemState {
        timestamp: Local::now().to_rfc3339(),
        total_memory,
        used_memory,
        free_memory,
        cpu_usage,
        disk_free,
    }
}

fn validate_system_state(state: &SystemState) -> Result<()> {
    if state.cpu_usage < 0.0 || state.cpu_usage > 100.0 {
        return Err(anyhow::anyhow!("Invalid CPU usage: {}", state.cpu_usage));
    }
    
    if state.total_memory == 0 {
        return Err(anyhow::anyhow!("Invalid total memory"));
    }
    
    if state.used_memory > state.total_memory {
        return Err(anyhow::anyhow!("Used memory exceeds total memory"));
    }
    
    if state.free_memory > state.total_memory {
        return Err(anyhow::anyhow!("Free memory exceeds total memory"));
    }
    
    Ok(())
}

fn validate_output_format(output: &str, expected_keywords: &[&str]) -> bool {
    if output.is_empty() {
        return false;
    }
    expected_keywords.iter().any(|keyword| output.contains(keyword))
}

fn log_behavior(command: &str, input: &str, output: &str, status: &str, duration_ms: u128, state_before: SystemState, state_after: SystemState) {
    let log_entry = BehaviorLog {
        timestamp: Local::now().to_rfc3339(),
        command: command.to_string(),
        input: input.to_string(),
        output: output.to_string(),
        status: status.to_string(),
        duration_ms,
        state_before,
        state_after,
    };
    
    if let Ok(mut logs) = BEHAVIOR_LOGS.lock() {
        logs.push(log_entry);
    }
}

fn get_behavior_logs() -> Vec<BehaviorLog> {
    BEHAVIOR_LOGS.lock().ok().map(|logs| logs.clone()).unwrap_or_default()
}

async fn handle_command_request(
    State(commands): State<Arc<HashMap<String, Box<dyn Fn(&str) -> String + Send + Sync>>>>,
    Json(payload): Json<CommandRequest>,
) -> Json<CommandResponse> {
    let start_time = std::time::Instant::now();
    let state_before = capture_system_state();
    
    let parts: Vec<&str> = payload.command.split_whitespace().collect();
    let command = parts[0];
    let argument = parts.get(1..).map(|args| args.join(" ")).unwrap_or_default();

    let response = commands
        .get(command)
        .map(|func| func(&argument))
        .unwrap_or_else(|| "Sorry, I didn't get you.".to_string());

    let state_after = capture_system_state();
    let duration_ms = start_time.elapsed().as_millis();
    let status = if response.contains("Error") { "FAILED" } else { "SUCCESS" };
    
    let _ = validate_system_state(&state_before);
    let _ = validate_system_state(&state_after);
    
    log_behavior(command, &argument, &response, status, duration_ms, state_before, state_after);

    Json(CommandResponse { response })
}

fn speak_to_speaker(text: &str) -> Result<()> {
    println!("{}", text);
    if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args(&[
                "-Command",
                &format!(
                    "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).Speak('{}')",
                    text
                ),
            ])
            .status()
            .context("Failed to speak")?;
    } else {
        Command::new("espeak")
            .arg(text)
            .status()
            .context("Failed to speak")?;
    }
    Ok(())
}

fn hello(_arg: &str) -> String {
    if let Err(e) = speak_to_speaker("Hello!") {
        return format!("Error: {}", e);
    }
    "Hello!".to_string()
}

fn who_created_you(_arg: &str) -> String {
    if let Err(e) = speak_to_speaker("Sutharsan and Nandhana.") {
        return format!("Error: {}", e);
    }
    "Sutharsan and Nandhana.".to_string()
}

fn current_day(_arg: &str) -> String {
    let day = Local::now().format("%A").to_string();
    if let Err(e) = speak_to_speaker(&format!("Today is {}", day)) {
        return format!("Error: {}", e);
    }
    day
}

fn current_date(_arg: &str) -> String {
    let date = Local::now().format("%d %m %Y").to_string();
    if let Err(e) = speak_to_speaker(&format!("Today's date is {}", date)) {
        return format!("Error: {}", e);
    }
    date
}

fn current_time(_arg: &str) -> String {
    let time = Local::now().format("%H:%M:%S").to_string();
    if let Err(e) = speak_to_speaker(&time) {
        return format!("Error: {}", e);
    }
    time
}

fn generate_calendar(year: i32, month: u32) -> String {
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let days_in_month = first_day
        .with_day(1)
        .unwrap()
        .with_day(31)
        .map_or_else(|| first_day.with_day(30), Some)
        .unwrap()
        .day();

    let mut calendar = format!("\n   {} {}\n", first_day.format("%B"), year);
    calendar.push_str("Su Mo Tu We Th Fr Sa\n");

    let start_weekday = first_day.weekday().num_days_from_sunday() as usize;
    let mut day = 1;

    for _ in 0..start_weekday {
        calendar.push_str("   ");
    }

    while day <= days_in_month {
        for weekday in 0..7 {
            if day > days_in_month {
                break;
            }
            if weekday < start_weekday && day == 1 {
                continue;
            }
            calendar.push_str(&format!("{:2} ", day));
            day += 1;
        }
        calendar.push('\n');
    }

    calendar
}

fn show_calendar(_arg: &str) -> String {
    let now = Local::now();
    let year = now.year();
    let month = now.month();
    let calendar_text = generate_calendar(year, month);

    if let Err(e) = speak_to_speaker(&format!("Here is the calendar for {} {}", now.format("%B"), year)) {
        return format!("Error: {}", e);
    }

    calendar_text
}

fn go_home(_arg: &str) -> String {
    if let Some(home) = home_dir() {
        if std::env::set_current_dir(&home).is_ok() {
            if let Err(e) = speak_to_speaker(&format!("Home directory: {}", home.display())) {
                return format!("Error: {}", e);
            }
            return format!("Home directory: {}", home.display());
        }
    }
    "Failed to change to home directory.".to_string()
}

fn root_directory(_arg: &str) -> String {
    let root = if cfg!(target_os = "windows") { "C:\\" } else { "/" };
    if std::env::set_current_dir(root).is_ok() {
        if let Err(e) = speak_to_speaker("You are now in the root directory.") {
            return format!("Error: {}", e);
        }
        return root.to_string();
    }
    "Failed to change to root directory.".to_string()
}

fn list_files(_arg: &str) -> String {
    match fs::read_dir(".") {
        Ok(entries) => {
            let files: Vec<String> = entries
                .filter_map(|entry| entry.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect();
            if let Err(e) = speak_to_speaker("The files in the current directory are listed.") {
                return format!("Error: {}", e);
            }
            files.join("\n")
        }
        Err(_) => "Failed to read directory.".to_string(),
    }
}

fn go_my_directory(_arg: &str) -> String {
    if let Some(home) = home_dir() {
        const MY_DIRECTORY: &str = "my_directory";
        let my_dir = home.join(MY_DIRECTORY);
       
        if fs::create_dir_all(&my_dir).is_err() {
            return "Failed to create your personal directory.".to_string();
        }

        if std::env::set_current_dir(&my_dir).is_ok() {
            let message = format!("You are now in your personal directory: {}", my_dir.display());
            if let Err(e) = speak_to_speaker(&message) {
                return format!("Error: {}", e);
            }
            return message;
        }
    }
    "Failed to navigate to your personal directory.".to_string()
}

fn current_directory(_arg: &str) -> String {
    match std::env::current_dir() {
        Ok(path) => {
            if let Err(e) = speak_to_speaker(&format!("You are in {}", path.display())) {
                return format!("Error: {}", e);
            }
            format!("You are in {}", path.display())
        },
        Err(_) => "Failed to get current directory.".to_string(),
    }
}

fn disk_usage(_arg: &str) -> String {
    let mut sys = System::new_all();
    sys.refresh_disks_list();
    let disk_info: Vec<String> = sys.disks()
        .iter()
        .map(|disk| {
            let total = disk.total_space() / (1024 * 1024 * 1024);
            let used = (disk.total_space() - disk.available_space()) / (1024 * 1024 * 1024);
            let free = disk.available_space() / (1024 * 1024 * 1024);
            format!(
                "Disk {}: Total: {} GB, Used: {} GB, Free: {} GB",
                disk.mount_point().display(), total, used, free
            )
        })
        .collect();
   
    let result = disk_info.join("\n");
    if let Err(e) = speak_to_speaker("Here is the disk usage report.") {
        return format!("Error: {}", e);
    }
    result
}

fn memory_usage(_arg: &str) -> String {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_memory = sys.total_memory() / (1024 * 1024);
    let used_memory = sys.used_memory() / (1024 * 1024);
    let free_memory = sys.free_memory() / (1024 * 1024);
    let result = format!(
        "Memory: Total: {} MB, Used: {} MB, Free: {} MB",
        total_memory, used_memory, free_memory
    );
    if let Err(e) = speak_to_speaker("Here is the memory usage report.") {
        return format!("Error: {}", e);
    }
    result
}

fn free_memory(_arg: &str) -> String {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let free = sys.free_memory() / 1024 / 1024;
    let result = format!("Free Memory: {} MB", free);
    if let Err(e) = speak_to_speaker(&result) {
        return format!("Error: {}", e);
    }
    result
}

fn swap_memory(_arg: &str) -> String {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let swap = sys.used_swap() / 1024 / 1024;
    let result = format!("Swap Memory: {} MB", swap);
    if let Err(e) = speak_to_speaker(&result) {
        return format!("Error: {}", e);
    }
    result
}

fn cpu_usage(_arg: &str) -> String {
    let mut sys = System::new_all();
    sys.refresh_cpu();
    let cpu_usage: Vec<String> = sys.cpus()
        .iter()
        .map(|cpu| format!("CPU {}: {}%", cpu.name(), cpu.cpu_usage()))
        .collect();
   
    let result = cpu_usage.join("\n");
    if let Err(e) = speak_to_speaker("Here is the CPU usage report.") {
        return format!("Error: {}", e);
    }
    result
}

fn run_command(command: &str, args: &[&str]) -> String {
    match Command::new(command).args(args).output() {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).to_string()
            } else {
                format!(
                    "Command failed with error: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
            }
        }
        Err(e) => format!("Failed to execute command: {}", e),
    }
}

fn ps_command(_arg: &str) -> String {
    let result = if cfg!(target_os = "windows") {
        run_command("tasklist", &[])
    } else {
        run_command("ps", &["aux"])
    };

    if let Err(e) = speak_to_speaker("Here is the list of running processes.") {
        return format!("Error: {}", e);
    }
    result
}

fn df_command(_arg: &str) -> String {
    let result = if cfg!(target_os = "windows") {
        run_command("wmic", &["logicaldisk", "get", "size,freespace,caption"])
    } else {
        run_command("df", &["-h"])
    };

    if let Err(e) = speak_to_speaker("Here is the disk space information.") {
        return format!("Error: {}", e);
    }
    result
}

fn list_users(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("net", &["user"])
    } else {
        run_command("cut", &["-d:", "-f1", "/etc/passwd"])
    }
}

fn list_services(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("tasklist", &[])
    } else {
        run_command("systemctl", &["list-units", "--type=service", "--all"])
    }
}

fn list_ports(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("netstat", &["-ano"])
    } else {
        run_command("netstat", &["-tulnp"])
    }
}

fn list_networks(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("ipconfig", &["/all"])
    } else {
        run_command("ifconfig", &[])
    }
}

fn list_drives(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("wmic", &["logicaldisk", "get", "caption"])
    } else {
        run_command("lsblk", &["-o", "NAME,MOUNTPOINT"])
    }
}

fn list_printers(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("wmic", &["printer", "get", "name"])
    } else {
        run_command("lpstat", &["-p"])
    }
}

fn list_disks(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("wmic", &["diskdrive", "get", "caption,size"])
    } else {
        run_command("lsblk", &["-o", "NAME,SIZE,TYPE"])
    }
}

fn list_folders(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        run_command("cmd", &["/C", "dir /AD /B"])
    } else {
        run_command("ls", &["-d", "*/"])
    }
}

fn help(_arg: &str) -> String {
    let commands = vec![
        "hello", "who_created_you", "help",
        "current_day", "current_date", "current_time", "show_calendar",
        "go_home", "root_directory", "go_my_directory", "current_directory", "navigate_directories",
        "list_files", "open_file", "create_file", "delete_file", "create_nano_file", "print_file_content",
        "create_symlink",
        "disk_usage", "memory_usage", "cpu_usage", "swap_memory", "free_memory", "df_command", "ps_command",
        "get_uptime",
        "list_processes", "list_services", "list_users",
        "list_ports", "list_networks",
        "list_drives", "list_disks", "list_printers", "list_folders",
        "volume_up", "volume_down",
        "compile_code", "run_code",
        "shutdown", "restart", "logout", "hibernate", "sleep",
        "command_history"
    ];

    let response = format!("Available commands:\n{}", commands.join("\n"));
    if let Err(e) = speak_to_speaker("Here are the available commands.") {
        return format!("Error: {}", e);
    }
    response
}

fn clean_filename(arg: &str) -> String {
    let arg = arg.replace("dot", ".");
    let arg = arg.split_whitespace().collect::<Vec<_>>().join(" ");
    arg.replace(" .", ".").replace(". ", ".").trim().to_string()
}

fn open_file(arg: &str) -> String {
    if arg.is_empty() {
        return "Error: Please provide a file name.".to_string();
    }
    let result = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", "start", "", arg])
            .status()
    } else {
        Command::new("xdg-open")
            .arg(arg)
            .status()
    };
    match result {
        Ok(_) => {
            if let Err(e) = speak_to_speaker(&format!("Opening {}", arg)) {
                return format!("Error: {}", e);
            }
            format!("Opening {}", arg)
        }
        Err(e) => format!("Failed to open file: {}", e),
    }
}

fn create_file(arg: &str) -> String {
    let cleaned_arg = clean_filename(arg);
    if cleaned_arg.is_empty() {
        return "Error: Please provide a valid file name.".to_string();
    }

    match fs::File::create(&cleaned_arg) {
        Ok(_) => {
            assert!(Path::new(&cleaned_arg).exists(), "File not created");
            format!("File '{}' created successfully.", cleaned_arg)
        },
        Err(e) => format!("Failed to create file '{}': {}", cleaned_arg, e),
    }
}

fn delete_file(arg: &str) -> String {
    let cleaned_arg = clean_filename(arg);

    if cleaned_arg.is_empty() {
        return "Error: Please provide a valid file name.".to_string();
    }

    match fs::remove_file(&cleaned_arg) {
        Ok(_) => {
            assert!(!Path::new(&cleaned_arg).exists(), "File still exists after deletion");
            format!("File '{}' deleted successfully.", cleaned_arg)
        },
        Err(e) => format!("Failed to delete file '{}': {}", cleaned_arg, e),
    }
}

fn move_file_or_folder(args: &str) -> String {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "Error: Please provide source and destination.".to_string();
    }

    let source = clean_filename(parts[0]);
    let destination = clean_filename(parts[1]);

    if !Path::new(&source).exists() {
        return format!("Error: Source '{}' does not exist.", source);
    }

    match fs::rename(&source, &destination) {
        Ok(_) => {
            assert!(Path::new(&destination).exists(), "Destination not found after move");
            assert!(!Path::new(&source).exists(), "Source still exists after move");
            format!("Moved '{}' to '{}'.", source, destination)
        },
        Err(e) => format!("Failed to move '{}': {}", source, e),
    }
}

fn rename_file_or_folder(args: &str) -> String {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() != 2 {
        return "Error: Please provide both old and new file names.".to_string();
    }

    let old_name = clean_filename(parts[0]);
    let new_name = clean_filename(parts[1]);

    match fs::rename(&old_name, &new_name) {
        Ok(_) => {
            assert!(Path::new(&new_name).exists(), "New file not found after rename");
            assert!(!Path::new(&old_name).exists(), "Old file still exists after rename");
            format!("Renamed: {} -> {}", old_name, new_name)
        },
        Err(e) => format!("Failed to rename: {}", e),
    }
}

fn search_file_or_folder(arg: &str) -> String {
    let cleaned_arg = clean_filename(arg);
    
    fn search_recursive(path: &Path, target: &str) -> Option<String> {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry.file_name().to_string_lossy().contains(target) {
                    return Some(entry_path.display().to_string());
                }
                if entry_path.is_dir() {
                    if let Some(found) = search_recursive(&entry_path, target) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    match search_recursive(Path::new("."), &cleaned_arg) {
        Some(found) => format!("Found: {}", found),
        None => format!("'{}' not found.", cleaned_arg),
    }
}

fn shutdown(_arg: &str) -> String {
    let confirmation = "Are you sure you want to shut down the PC? (Assuming yes for now)";
    if let Err(e) = speak_to_speaker(confirmation) {
        return format!("Error: {}", e);
    }
    if let Err(e) = speak_to_speaker("Shutting down the PC.") {
        return format!("Error: {}", e);
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("shutdown").args(&["/s", "/t", "0"]).status()
    } else {
        Command::new("shutdown").args(&["-h", "now"]).status()
    };
    match status {
        Ok(_) => "Shutdown command executed.".to_string(),
        Err(e) => format!("Failed to execute shutdown: {}", e),
    }
}

fn restart(_arg: &str) -> String {
    if let Err(e) = speak_to_speaker("Restarting the system.") {
        return format!("Error: {}", e);
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("shutdown").args(&["/r", "/t", "0"]).status()
    } else {
        Command::new("shutdown").args(&["-r", "now"]).status()
    };
    match status {
        Ok(_) => "Restart command executed.".to_string(),
        Err(e) => format!("Failed to restart: {}", e),
    }
}

fn logout(_arg: &str) -> String {
    if let Err(e) = speak_to_speaker("Logging out.") {
        return format!("Error: {}", e);
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("shutdown").args(&["/l"]).status()
    } else {
        Command::new("pkill").arg("-KILL").arg("-u").arg(whoami::username()).status()
    };
    match status {
        Ok(_) => "Logout command executed.".to_string(),
        Err(e) => format!("Failed to log out: {}", e),
    }
}

fn hibernate(_arg: &str) -> String {
    if let Err(e) = speak_to_speaker("Hibernating system.") {
        return format!("Error: {}", e);
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("shutdown").args(&["/h"]).status()
    } else {
        Command::new("systemctl").arg("hibernate").status()
    };
    match status {
        Ok(_) => "Hibernate command executed.".to_string(),
        Err(e) => format!("Failed to hibernate: {}", e),
    }
}

fn sleep(_arg: &str) -> String {
    if let Err(e) = speak_to_speaker("Putting system to sleep.") {
        return format!("Error: {}", e);
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("rundll32.exe").args(&["powrprof.dll,SetSuspendState", "0", "1", "0"]).status()
    } else {
        Command::new("systemctl").arg("suspend").status()
    };
    match status {
        Ok(_) => "Sleep command executed.".to_string(),
        Err(e) => format!("Failed to sleep: {}", e),
    }
}

fn volume_up(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        match Command::new("powershell")
            .args(&["-Command", "(New-Object -ComObject WScript.Shell).SendKeys([char]175)"])
            .status()
        {
            Ok(_) => {
                if let Err(e) = speak_to_speaker("Volume increased.") {
                    return format!("Error: {}", e);
                }
                "Volume increased.".to_string()
            }
            Err(e) => format!("Failed to increase volume: {}", e),
        }
    } else {
        if let Err(e) = speak_to_speaker("Volume control not supported on this OS yet.") {
            return format!("Error: {}", e);
        }
        "Volume control not supported on this OS yet.".to_string()
    }
}

fn volume_down(_arg: &str) -> String {
    if cfg!(target_os = "windows") {
        match Command::new("powershell")
            .args(&["-Command", "(New-Object -ComObject WScript.Shell).SendKeys([char]174)"])
            .status()
        {
            Ok(_) => {
                if let Err(e) = speak_to_speaker("Volume decreased.") {
                    return format!("Error: {}", e);
                }
                "Volume decreased.".to_string()
            }
            Err(e) => format!("Failed to decrease volume: {}", e),
        }
    } else {
        if let Err(e) = speak_to_speaker("Volume control not supported on this OS yet.") {
            return format!("Error: {}", e);
        }
        "Volume control not supported on this OS yet.".to_string()
    }
}

fn compile_code(arg: &str) -> String {
    let filename = clean_filename(arg);
    if filename.is_empty() {
        return "Error: Please provide a file name to compile.".to_string();
    }

    let output_name = filename.trim_end_matches(".c").trim_end_matches(".cpp");

    let output = if filename.ends_with(".rs") {
        Command::new("rustc").arg(&filename).output()
    } else if filename.ends_with(".c") {
        Command::new("gcc").args(&[&filename, "-o", output_name]).output()
    } else if filename.ends_with(".cpp") {
        Command::new("g++").args(&[&filename, "-o", output_name]).output()
    } else if filename.ends_with(".py") {
        Command::new("python3").args(&["-m", "py_compile", &filename]).output()
    } else {
        return "Error: Unsupported file format.".to_string();
    };

    match output {
        Ok(output) => {
            if output.status.success() {
                format!("Compilation successful. Executable: {}", output_name)
            } else {
                format!("Compilation failed: {}", String::from_utf8_lossy(&output.stderr))
            }
        }
        Err(e) => format!("Failed to compile: {}", e),
    }
}

fn print_file_content(arg: &str) -> String {
    let filename = clean_filename(arg);
    if filename.is_empty() {
        return "Error: Please provide a file name.".to_string();
    }
    
    match fs::File::open(&filename) {
        Ok(file) => {
            let reader = BufReader::new(file);
            let mut content = String::new();
            for line in reader.lines() {
                if let Ok(line) = line {
                    content.push_str(&line);
                    content.push('\n');
                }
            }
            content
        }
        Err(e) => format!("Failed to open file '{}': {}", filename, e),
    }
}

fn run_code(arg: &str) -> String {
    let cleaned_arg = clean_filename(arg);
    let executable = cleaned_arg.trim_end_matches(".c").trim_end_matches(".cpp");

    let output = if cleaned_arg.ends_with(".py") {
        Command::new("python3").arg(&cleaned_arg).output()
    } else {
        Command::new(format!("./{}", executable)).output()
    };

    match output {
        Ok(output) => {
            if output.status.success() {
                format!("Output:\n{}", String::from_utf8_lossy(&output.stdout))
            } else {
                format!("Execution failed: {}", String::from_utf8_lossy(&output.stderr))
            }
        }
        Err(e) => format!("Error running the program: {}", e),
    }
}

fn create_symlink(arg: &str) -> String {
    let args: Vec<&str> = arg.split_whitespace().collect();
    if args.len() < 2 {
        return "Error: Please provide source and target (e.g., 'source target').".to_string();
    }
    let source = args[0];
    let target = args[1];
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", "mklink", target, source])
            .status()
    } else {
        Command::new("ln")
            .args(&["-s", source, target])
            .status()
    };
    match status {
        Ok(_) => {
            if let Err(e) = speak_to_speaker(&format!("Created symlink from {} to {}", source, target)) {
                return format!("Error: {}", e);
            }
            format!("Created symlink from {} to {}", source, target)
        }
        Err(e) => format!("Failed to create symlink: {}", e),
    }
}

fn navigate_directories(_arg: &str) -> String {
    "Error: Interactive navigation not supported via API yet.".to_string()
}

fn get_uptime(_arg: &str) -> String {
    let mut sys = System::new_all();
    sys.refresh_system();
    let uptime = sys.uptime();
    let duration = Duration::seconds(uptime as i64);
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;
    let result = format!("Uptime: {}h {}m {}s", hours, minutes, seconds);
    if let Err(e) = speak_to_speaker(&result) {
        return format!("Error: {}", e);
    }
    result
}

fn command_history(_arg: &str) -> String {
    match std::fs::read_to_string("history.txt") {
        Ok(history) => {
            if let Err(e) = speak_to_speaker("Command history displayed.") {
                return format!("Error: {}", e);
            }
            history
        }
        Err(_) => {
            if let Err(e) = speak_to_speaker("No command history found.") {
                return format!("Error: {}", e);
            }
            "No command history found.".to_string()
        }
    }
}

fn exit(_arg: &str) -> String {
    if let Err(e) = speak_to_speaker("Goodbye!") {
        return format!("Error: {}", e);
    }
    println!("Goodbye!");
    std::thread::sleep(std::time::Duration::from_secs(1));
    process::exit(0);
}

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut commands: HashMap<String, Box<dyn Fn(&str) -> String + Send + Sync>> = HashMap::new();
    commands.insert("hello".to_string(), Box::new(hello));
    commands.insert("who_created_you".to_string(), Box::new(who_created_you));
    commands.insert("current_day".to_string(), Box::new(current_day));
    commands.insert("current_date".to_string(), Box::new(current_date));
    commands.insert("current_time".to_string(), Box::new(current_time));
    commands.insert("show_calendar".to_string(), Box::new(show_calendar));
    commands.insert("go_home".to_string(), Box::new(go_home));
    commands.insert("root_directory".to_string(), Box::new(root_directory));
    commands.insert("list_files".to_string(), Box::new(list_files));
    commands.insert("current_directory".to_string(), Box::new(current_directory));
    commands.insert("go_my_directory".to_string(), Box::new(go_my_directory));
    commands.insert("disk_usage".to_string(), Box::new(disk_usage));
    commands.insert("memory_usage".to_string(), Box::new(memory_usage));
    commands.insert("swap_memory".to_string(), Box::new(swap_memory));
    commands.insert("free_memory".to_string(), Box::new(free_memory));
    commands.insert("cpu_usage".to_string(), Box::new(cpu_usage));
    commands.insert("ps_command".to_string(), Box::new(ps_command));
    commands.insert("df_command".to_string(), Box::new(df_command));
    commands.insert("shutdown".to_string(), Box::new(shutdown));
    commands.insert("restart".to_string(), Box::new(restart));
    commands.insert("logout".to_string(), Box::new(logout));
    commands.insert("hibernate".to_string(), Box::new(hibernate));
    commands.insert("sleep".to_string(), Box::new(sleep));
    commands.insert("help".to_string(), Box::new(help));
    commands.insert("list_users".to_string(), Box::new(list_users));
    commands.insert("list_services".to_string(), Box::new(list_services));
    commands.insert("list_ports".to_string(), Box::new(list_ports));
    commands.insert("list_networks".to_string(), Box::new(list_networks));
    commands.insert("list_drives".to_string(), Box::new(list_drives));
    commands.insert("list_printers".to_string(), Box::new(list_printers));
    commands.insert("list_disks".to_string(), Box::new(list_disks));
    commands.insert("list_folders".to_string(), Box::new(list_folders));
    commands.insert("open_file".to_string(), Box::new(open_file));
    commands.insert("create_file".to_string(), Box::new(create_file));
    commands.insert("delete_file".to_string(), Box::new(delete_file));
    commands.insert("print_file_content".to_string(), Box::new(print_file_content));
    commands.insert("create_symlink".to_string(), Box::new(create_symlink));
    commands.insert("search_file_or_folder".to_string(), Box::new(search_file_or_folder));
    commands.insert("volume_up".to_string(), Box::new(volume_up));
    commands.insert("volume_down".to_string(), Box::new(volume_down));
    commands.insert("compile_code".to_string(), Box::new(compile_code));
    commands.insert("run_code".to_string(), Box::new(run_code));
    commands.insert("navigate_directories".to_string(), Box::new(navigate_directories));
    commands.insert("get_uptime".to_string(), Box::new(get_uptime));
    commands.insert("command_history".to_string(), Box::new(command_history));
    commands.insert("exit".to_string(), Box::new(exit));

    let app = Router::new()
        .route("/backend", get(|| async { "Hello from Rust backend!" }))
        .route("/command", post(handle_command_request))
        .route("/logs", get(|| async { Json(get_behavior_logs()) }))
        .layer(cors)
        .with_state(Arc::new(commands));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    println!("🚀 Backend running at http://{}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_system_state_valid() {
        let state = SystemState {
            timestamp: Local::now().to_rfc3339(),
            total_memory: 8000,
            used_memory: 4000,
            free_memory: 4000,
            cpu_usage: 50.0,
            disk_free: 100000,
        };
        assert!(validate_system_state(&state).is_ok());
    }

    #[test]
    fn test_validate_system_state_invalid_cpu() {
        let state = SystemState {
            timestamp: Local::now().to_rfc3339(),
            total_memory: 8000,
            used_memory: 4000,
            free_memory: 4000,
            cpu_usage: 150.0,
            disk_free: 100000,
        };
        assert!(validate_system_state(&state).is_err());
    }

    #[test]
    fn test_validate_system_state_memory_exceeds() {
        let state = SystemState {
            timestamp: Local::now().to_rfc3339(),
            total_memory: 8000,
            used_memory: 9000,
            free_memory: 4000,
            cpu_usage: 50.0,
            disk_free: 100000,
        };
        assert!(validate_system_state(&state).is_err());
    }

    #[test]
    fn test_validate_output_format_valid() {
        let output = "Memory: Total: 8000 MB, Used: 4000 MB, Free: 4000 MB";
        assert!(validate_output_format(output, &["Memory", "MB"]));
    }

    #[test]
    fn test_validate_output_format_invalid() {
        let output = "Some random output";
        assert!(!validate_output_format(output, &["Memory", "MB"]));
    }

    #[test]
    fn test_validate_output_format_empty() {
        let output = "";
        assert!(!validate_output_format(output, &["Memory"]));
    }

    #[test]
    fn test_clean_filename_with_dot() {
        let result = clean_filename("test dot txt");
        assert_eq!(result, "test.txt");
    }

    #[test]
    fn test_clean_filename_multiple_spaces() {
        let result = clean_filename("test   file   name");
        assert_eq!(result, "test file name");
    }

    #[test]
    fn test_clean_filename_empty() {
        let result = clean_filename("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_create_file_success() {
        let filename = "test_file_12345.txt";
        let result = create_file(filename);
        assert!(result.contains("created successfully"));
        assert!(Path::new(filename).exists());
        let _ = fs::remove_file(filename);
    }

    #[test]
    fn test_create_file_empty_name() {
        let result = create_file("");
        assert!(result.contains("Error"));
    }

    #[test]
    fn test_delete_file_success() {
        let filename = "test_delete_12345.txt";
        let _ = fs::File::create(filename);
        let result = delete_file(filename);
        assert!(result.contains("deleted successfully"));
        assert!(!Path::new(filename).exists());
    }

    #[test]
    fn test_delete_file_not_exists() {
        let result = delete_file("nonexistent_file_12345.txt");
        assert!(result.contains("Failed"));
    }

    #[test]
    fn test_move_file_success() {
        let source = "source_file_12345.txt";
        let dest = "dest_file_12345.txt";
        let _ = fs::File::create(source);
        let result = move_file_or_folder(&format!("{} {}", source, dest));
        assert!(result.contains("Moved"));
        assert!(Path::new(dest).exists());
        let _ = fs::remove_file(dest);
    }

    #[test]
    fn test_move_file_source_not_exists() {
        let result = move_file_or_folder("nonexistent_12345.txt dest.txt");
        assert!(result.contains("Error"));
    }

    #[test]
    fn test_rename_file_success() {
        let old_name = "old_name_12345.txt";
        let new_name = "new_name_12345.txt";
        let _ = fs::File::create(old_name);
        let result = rename_file_or_folder(&format!("{} {}", old_name, new_name));
        assert!(result.contains("Renamed"));
        assert!(Path::new(new_name).exists());
        let _ = fs::remove_file(new_name);
    }

    #[test]
    fn test_capture_system_state() {
        let state = capture_system_state();
        assert!(state.total_memory > 0);
        assert!(state.cpu_usage >= 0.0 && state.cpu_usage <= 100.0);
        assert!(!state.timestamp.is_empty());
    }

    #[test]
    fn test_log_behavior() {
        let state = SystemState {
            timestamp: Local::now().to_rfc3339(),
            total_memory: 8000,
            used_memory: 4000,
            free_memory: 4000,
            cpu_usage: 50.0,
            disk_free: 100000,
        };
        
        log_behavior("test_cmd", "test_input", "test_output", "SUCCESS", 100, state.clone(), state);
        let logs = get_behavior_logs();
        assert!(!logs.is_empty());
        assert_eq!(logs.last().unwrap().command, "test_cmd");
        assert_eq!(logs.last().unwrap().status, "SUCCESS");
    }

    #[test]
    fn test_hello_command() {
        let result = hello("");
        assert_eq!(result, "Hello!");
    }

    #[test]
    fn test_current_day_format() {
        let result = current_day("");
        assert!(!result.is_empty());
        let valid_days = vec!["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
        assert!(valid_days.iter().any(|day| result.contains(day)));
    }

    #[test]
    fn test_current_time_format() {
        let result = current_time("");
        assert!(result.contains(":"));
    }

    #[test]
    fn test_memory_usage_format() {
        let result = memory_usage("");
        assert!(validate_output_format(&result, &["Memory", "MB"]));
    }

    #[test]
    fn test_cpu_usage_format() {
        let result = cpu_usage("");
        assert!(validate_output_format(&result, &["CPU", "%"]));
    }

    #[test]
    fn test_disk_usage_format() {
        let result = disk_usage("");
        assert!(validate_output_format(&result, &["Disk", "GB"]));
    }

    #[test]
    fn test_edge_case_very_long_filename() {
        let long_name = "a".repeat(200);
        let result = clean_filename(&long_name);
        assert_eq!(result.len(), 200);
    }

    #[test]
    fn test_edge_case_special_characters() {
        let result = clean_filename("test@#$%file");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_boundary_zero_memory() {
        let state = SystemState {
            timestamp: Local::now().to_rfc3339(),
            total_memory: 0,
            used_memory: 0,
            free_memory: 0,
            cpu_usage: 50.0,
            disk_free: 100000,
        };
        assert!(validate_system_state(&state).is_err());
    }

    #[test]
    fn test_boundary_max_cpu() {
        let state = SystemState {
            timestamp: Local::now().to_rfc3339(),
            total_memory: 8000,
            used_memory: 4000,
            free_memory: 4000,
            cpu_usage: 100.0,
            disk_free: 100000,
        };
        assert!(validate_system_state(&state).is_ok());
    }

    #[test]
    fn test_boundary_min_cpu() {
        let state = SystemState {
            timestamp: Local::now().to_rfc3339(),
            total_memory: 8000,
            used_memory: 4000,
            free_memory: 4000,
            cpu_usage: 0.0,
            disk_free: 100000,
        };
        assert!(validate_system_state(&state).is_ok());
    }
}

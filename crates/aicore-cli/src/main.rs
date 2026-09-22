use aicore_protocol::{Request, Response, PROTOCOL_VERSION};
use clap::{Parser, Subcommand};
use std::os::unix::net::UnixStream;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "aicore")]
#[command(about = "AICore CLI tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Status,
    Auth,
    Logout,
    Logs,
    Stop,
    Restart,
    Version,
}

fn determine_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("aicore.sock")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".ai-core").join("aicore.sock")
    }
}

fn send_socket_request(method: &str, params: serde_json::Value) -> Result<Response, String> {
    let socket_path = determine_socket_path();
    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("Could not connect to AICore socket at {}: {}", socket_path.display(), e))?;

    let req = Request::new("cli_req_1", method, params);
    let mut json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    json.push('\n');

    stream.write_all(json.as_bytes()).map_err(|e| e.to_string())?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).map_err(|e| e.to_string())?;

    let resp: Response = serde_json::from_str(response_line.trim())
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(resp)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Status => {
            println!("AICore Client CLI v{}", PROTOCOL_VERSION);
            match send_socket_request("auth.status", serde_json::json!({})) {
                Ok(resp) => println!("Service Response: {:?}", resp),
                Err(e) => println!("Status Error: {}", e),
            }
        }
        Commands::Auth => {
            match send_socket_request("auth.login", serde_json::json!({})) {
                Ok(resp) => println!("Auth Login: {:?}", resp),
                Err(e) => println!("Auth Error: {}", e),
            }
        }
        Commands::Logout => {
            match send_socket_request("auth.logout", serde_json::json!({})) {
                Ok(resp) => println!("Logout Result: {:?}", resp),
                Err(e) => println!("Logout Error: {}", e),
            }
        }
        Commands::Logs => {
            println!("AICore logs directory: ~/.ai-core/logs");
        }
        Commands::Stop => {
            println!("Stopping AICore service...");
        }
        Commands::Restart => {
            println!("Restarting AICore service...");
        }
        Commands::Version => {
            println!("AICore v0.1.0 (Protocol v{})", PROTOCOL_VERSION);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_socket_path() {
        let path = determine_socket_path();
        assert!(path.to_string_lossy().contains("aicore.sock"));
    }
}

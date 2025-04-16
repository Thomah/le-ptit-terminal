use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use webbrowser;
use log::{debug, info, error};
use dirs;

const CONFIG_FILE: &str = ".les_ptits_gilets_config.json";
const REDIRECT_URI: &str = "http://localhost:5000/callback";
const TOKEN_URL: &str = "https://www.eventbrite.com/oauth/token";

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    token_info: Option<TokenInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TokenInfo {
    access_token: String,
    #[serde(default)]
    created_at: u64,
}

fn get_config_path() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to find home directory")
        .join(CONFIG_FILE)
}

pub fn load_config() -> Result<Config> {
    let path = get_config_path();
    if path.exists() {
        let data = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&data)?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = get_config_path();
    let data = serde_json::to_string_pretty(config)?;
    fs::write(path, data)?;
    Ok(())
}

pub fn get_access_token() -> Result<String> {
    debug!("Fetching access token");
    let mut config = load_config()?;

    if let Some(token_info) = &config.token_info {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if now - token_info.created_at < 3600 {
            debug!("Using cached token");
            return Ok(token_info.access_token.clone());
        }
    }

    let client_id = config.client_id.clone().ok_or_else(|| {
        error!("CLIENT_ID not set");
        anyhow!("CLIENT_ID not set")
    })?;
    let client_secret = config.client_secret.clone().ok_or_else(|| {
        error!("CLIENT_SECRET not set");
        anyhow!("CLIENT_SECRET not set")
    })?;

    let code = request_user_authorization(&client_id)?;
    let token = exchange_code_for_token(&client_id, &client_secret, &code)?;

    config.token_info = Some(token.clone());
    save_config(&config)?;

    debug!("Access token fetched successfully");
    Ok(token.access_token)
}

fn request_user_authorization(client_id: &str) -> Result<String> {
    debug!("Requesting user authorization...");
    let auth_url = format!(
        "https://www.eventbrite.com/oauth/authorize?response_type=code&client_id={}&redirect_uri={}",
        client_id, REDIRECT_URI
    );

    info!("🌐 Opening browser for authorization...");
    if let Err(e) = webbrowser::open(&auth_url) {
        error!("Failed to open browser for authorization: {}", e);
        return Err(anyhow!("Failed to open browser for authorization").into());
    }

    let auth_code = Arc::new(Mutex::new(None));
    let auth_code_clone = Arc::clone(&auth_code);

    let server_thread = thread::spawn(move || {
        if let Ok(listener) = TcpListener::bind("127.0.0.1:5000") {
            debug!("Listening for incoming HTTP requests on port 5000...");
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let mut buffer = [0; 1024];
                    if let Ok(size) = stream.read(&mut buffer) {
                        let request = String::from_utf8_lossy(&buffer[..size]);
                        if request.contains("GET /callback?code=") {
                            if let Some(start) = request.find("code=") {
                                let code = request[start + 5..]
                                    .split_whitespace()
                                    .next()
                                    .unwrap_or("")
                                    .split('&')
                                    .next()
                                    .unwrap_or("")
                                    .to_string();

                                debug!("Authorization code received: {}", code);
                                let response = b"HTTP/1.1 200 OK\r\n\r\nAuthorization successful. You may close this window.";
                                stream.write_all(response).unwrap();
                                *auth_code_clone.lock().unwrap() = Some(code);
                                break;
                            }
                        }
                    }
                }
            }
        }
    });

    server_thread.join().unwrap();

    let code_guard = auth_code.lock().unwrap();
    debug!("Authorization code retrieved: {:?}", code_guard);
    code_guard.clone().ok_or(anyhow!("No authorization code received."))
}

fn exchange_code_for_token(client_id: &str, client_secret: &str, code: &str) -> Result<TokenInfo> {
    debug!("Exchanging authorization code for access token...");
    let client = reqwest::blocking::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("redirect_uri", REDIRECT_URI),
    ];

    let resp = client.post(TOKEN_URL).form(&params).send()?;

    // Check if the response is successful
    if !resp.status().is_success() {
        let resp_text = resp.text()?;  // Read the response text only after status check
        error!("Failed to exchange token: {}", resp_text);
        return Err(anyhow!("Failed to exchange token: {}", resp_text));
    }

    // Now that we've checked the status, read the response text
    let resp_text = resp.text()?;
    
    // Deserialize the response into TokenInfo
    let mut token: TokenInfo = serde_json::from_str(&resp_text)?;
    token.created_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    
    info!("🔓 Token obtained and cached.");
    debug!("Token received: {:?}", token);
    
    Ok(token)
}

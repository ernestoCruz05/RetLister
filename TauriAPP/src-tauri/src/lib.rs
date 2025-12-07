// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn check_api_status(url: String) -> bool {
    let client = reqwest::Client::new();
    let token = std::env::var("RETLISTER_API_TOKEN")
        .unwrap_or_else(|_| "dev-token".to_string());
    
    let target_url = if url.ends_with("/health") {
        url
    } else {
        format!("{}/health", url.trim_end_matches('/'))
    };

    let response = client
        .get(&target_url)
        .header(USER_AGENT, "RetListerTauri/1.0")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await;

    match response {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

#[tauri::command]
async fn authenticated_request(method: String, url: String, body: Option<String>) -> Result<String, String> {
    let client = reqwest::Client::new();
    let token = std::env::var("RETLISTER_API_TOKEN")
        .unwrap_or_else(|_| "dev-token".to_string());

    let mut request = match method.as_str() {
        "POST" => client.post(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    };

    request = request
        .header(USER_AGENT, "RetListerTauri/1.0")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json");

    if let Some(b) = body {
        request = request.body(b);
    }

    let response = request.send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Request failed: {}", response.status()));
    }

    response.text().await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, check_api_status, authenticated_request])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use reqwest::header::USER_AGENT;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn check_api_status(url: String) -> bool {
    let client = reqwest::Client::new();
    
    let target_url = if url.ends_with("/health") {
        url
    } else {
        format!("{}/health", url.trim_end_matches('/'))
    };

    let response = client
        .get(&target_url)
        .header(USER_AGENT, "RetListerTauri/1.0")
        .send()
        .await;

    match response {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, check_api_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

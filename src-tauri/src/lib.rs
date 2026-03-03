use std::fs::{File, self};
use std::io::{copy, Cursor};
use std::path::PathBuf;
use futures_util::StreamExt;
use serde::Serialize;
use tauri::{Emitter, Window};
use std::path::Path;
use std::{thread, time::Duration};

use std::process::Command;
use std::os::windows::process::CommandExt;

#[derive(Clone, Serialize)]
struct ProgressPayload {
    progress: u64,
    total: u64,
    status: String,
}

use serde::Deserialize;
use reqwest::header::USER_AGENT;

#[derive(Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    browser_download_url: String,
    name: String,
}

#[derive(Debug)]
pub struct AssetInfo {
    pub url: String,
    pub name: String,
}

#[derive(Debug)]
pub struct RepositoryInfo {
    pub repo: String,
    pub what_update: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Config {
    last_version: String,
    last_preset: String,
    game_filter: bool,
    auto_start: bool,
}

async fn get_latest_download_url(repo: &str) -> Result<AssetInfo, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let client = reqwest::Client::new();
    
    let response = client
        .get(url)
        .header(USER_AGENT, "tauri-updater-app") // user agent
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let release: GithubRelease = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let asset = release.assets
        .iter()
        .find(|a| a.name.ends_with(".zip"))
        .ok_or("ZIP-файл не найден в релизе")?;

    Ok(AssetInfo {
        url: asset.browser_download_url.clone(),
        name: asset.name.clone(),
    })
}

pub fn update_resources(target_dir: &str) -> Result<(), String> {
    let resources_dir = Path::new("app/resources/list-general.txt");
    if !resources_dir.exists() || fs::metadata(resources_dir).map_err(|e| e.to_string())?.len() == 0 {
        fs::create_dir_all(resources_dir.parent().unwrap()).map_err(|e| e.to_string())?;
        File::create(resources_dir).map_err(|e| e.to_string())?;
        fs::copy(format!("{}/lists/list-general.txt", target_dir), resources_dir).map_err(|e| e.to_string())?;
        println!("Ресурсы обновлены: {}", format!("{}/lists/list-general.txt", target_dir));
    } else {
        println!("Ресурсы уже обновлены: {}", resources_dir.display());
    }
    Ok(())
} 

fn copy_sites_list(last_version: &String) {
    let path = Path::new("app/resources/list-general.txt");
    let download_dir = Path::new("app/downloads/").join(&last_version).join("lists/list-general.txt");
    if path.exists() {
        println!("Copying sites list from {} to {}", path.display(), download_dir.display());
        fs::copy(path, download_dir).unwrap();
    } else {
        File::create(path).unwrap();
        fs::copy(download_dir, path).unwrap();        
    } 
}

fn update_config(last_version: &String, target_dir: &String) {
    let config_data = fs::read_to_string("app/resources/config.json")
        .expect("Не удалось найти resources/config.json");
    
    let mut config: Config = serde_json::from_str(&config_data)
        .expect("Ошибка в формате config.json");

    config.last_version = last_version.to_string();
    let base_path = target_dir.to_owned() + "/utils/";

    if config.game_filter {
        let target_path = base_path.to_owned() + "game_filter.enabled";
        let file = File::create(&target_path);
        fs::write(target_path, "ENABLED");
    }

    copy_sites_list(&config.last_version);

    let data = serde_json::to_string_pretty(&config)
        .expect("Не удалось сохранить конфиг");
    fs::write("app/resources/config.json", data);
}

fn create_config() {
    let path = Path::new("app/resources/config.json");
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string());
        }
        let data = r#"{"last_version": "", "last_preset": "", "game_filter": true, "auto_start": false}"#;
        let _ = fs::write(path, data).map_err(|e| e.to_string());
    }
}

async fn get_latest_download_archive(window: &Window, repository_info: RepositoryInfo) -> Result<String, String> {
    let asset_info = get_latest_download_url(&repository_info.repo).await?;
    let url = asset_info.url;

    println!("Найдена ссылка: {}", url);
    let asset_name = &asset_info.name.replace(".zip", "");
    let target_dir = String::from("app/downloads/".to_owned() + &asset_name);

    if Path::new(&target_dir).exists() {
        println!("Файл обновлён до последней версии: {}", target_dir);
        window.emit("download-progress", ProgressPayload {
            progress: 100,
            total: 100,
            status: String::from("Файл обновлён до последней версии: ".to_owned() + &repository_info.what_update),
        }).unwrap();

        return Ok(target_dir);
    }

    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    let total_size = response.content_length().unwrap_or(0);
    
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        buffer.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;

        window.emit("download-progress", ProgressPayload {
            progress: downloaded,
            total: total_size,
            status: String::from("Загрузка... ".to_owned() + &repository_info.what_update),
        }).unwrap();
    }

    window.emit("download-progress", ProgressPayload {
        progress: 100,
        total: 100,
        status: String::from("Распаковка... ".to_owned() + &repository_info.what_update),
    }).unwrap();

    let reader = Cursor::new(buffer);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = PathBuf::from(&target_dir).join(file.name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).unwrap();
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p).unwrap();
            }
            let mut outfile = File::create(&outpath).unwrap();
            copy(&mut file, &mut outfile).unwrap();
        }
    }

    update_config(&asset_name, &target_dir);

    Ok(target_dir)
}

#[tauri::command]
async fn start_update(window: Window) -> Result<(), String> {
    let zapret_repo = RepositoryInfo {
        repo: "Flowseal/zapret-discord-youtube".into(),
        what_update: "zapret-discord-youtube".into(),
    };
    let zapret_ui_repo = RepositoryInfo {
        repo: "qubicfire/zapret-ui".into(),
        what_update: "zapret-ui".into(),
    };
    let target_dir = get_latest_download_archive(&window, zapret_repo).await?;

    update_resources(&target_dir)?;

    /*let executable_path = Path::new("app/zapret-ui.exe");
    if executable_path.exists() {
        Command::new(executable_path)
            .creation_flags(0x00000008) // DETACHED_PROCESS
            .spawn()
            .map_err(|e| e.to_string())?;
    } else {
        return Err("Исполняемый файл не найден после распаковки".into());
    }*/

    //thread::sleep(Duration::from_secs(1));

    window.emit("update-finished", "Готово!").unwrap();
    //window.close().unwrap();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    create_config();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![start_update])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

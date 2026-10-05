use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum UpdateEvent {
    UpdateAvailable {
        version: String,
        html_url: String,
        download_url: String,
    },
    UpToDate,
    CheckFailed(String),
    UpdateApplied,
    InstallFailed(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    Available {
        version: String,
        html_url: String,
        download_url: String,
    },
    UpToDate,
    Downloading,
    Applied,
    Failed(String),
}

pub fn spawn_update_check(tx: Sender<UpdateEvent>) {
    std::thread::spawn(move || {
        let result = (|| -> Result<(String, String, String), String> {
            let client = reqwest::blocking::Client::builder()
                .user_agent("mta-updater")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())?;

            let resp: serde_json::Value = client
                .get("https://api.github.com/repos/salernoelia/mta/releases/latest")
                .send()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())?;

            let tag = resp["tag_name"]
                .as_str()
                .ok_or("Missing tag_name in release response")?
                .trim_start_matches('v')
                .to_string();

            let html_url = resp["html_url"]
                .as_str()
                .ok_or("Missing html_url in release response")?
                .to_string();

            let asset_name = platform_asset_name();
            let download_url = resp["assets"]
                .as_array()
                .and_then(|assets| {
                    assets
                        .iter()
                        .find(|a| a["name"].as_str().map(|n| n == asset_name).unwrap_or(false))
                })
                .and_then(|a| a["browser_download_url"].as_str())
                .unwrap_or("")
                .to_string();

            Ok((tag, html_url, download_url))
        })();

        match result {
            Ok((latest, html_url, download_url)) => {
                let current = env!("CARGO_PKG_VERSION");
                if latest != current && !download_url.is_empty() {
                    let _ = tx.send(UpdateEvent::UpdateAvailable {
                        version: latest,
                        html_url,
                        download_url,
                    });
                } else {
                    let _ = tx.send(UpdateEvent::UpToDate);
                }
            }
            Err(e) => {
                let _ = tx.send(UpdateEvent::CheckFailed(e));
            }
        }
    });
}

pub fn spawn_self_update(download_url: String, tx: Sender<UpdateEvent>) {
    std::thread::spawn(move || {
        match perform_self_update(&download_url) {
            Ok(()) => {
                let _ = tx.send(UpdateEvent::UpdateApplied);
            }
            Err(e) => {
                let _ = tx.send(UpdateEvent::InstallFailed(e));
            }
        }
    });
}

fn platform_asset_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "mta-macos.app.tar.gz"
    }
    #[cfg(target_os = "linux")]
    {
        "mta-linux"
    }
    #[cfg(target_os = "windows")]
    {
        "mta-windows.exe"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "mta"
    }
}

fn perform_self_update(download_url: &str) -> Result<(), String> {
    let tmp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        let archive_path = tmp_dir.path().join("mta-update.tar.gz");
        let mut archive_file = std::fs::File::create(&archive_path).map_err(|e| e.to_string())?;
        let client = reqwest::blocking::Client::builder()
            .user_agent("mta-updater")
            .build()
            .map_err(|e| e.to_string())?;
        let bytes = client
            .get(download_url)
            .send()
            .and_then(|r| r.bytes())
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut bytes.as_ref(), &mut archive_file).map_err(|e| e.to_string())?;

        let file = std::fs::File::open(&archive_path).map_err(|e| e.to_string())?;
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);

        let extract_to = tmp_dir.path().join("mta_bin");
        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let entry_path = entry.path().map_err(|e| e.to_string())?;
            let file_name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if file_name == "mta" {
                entry.unpack(&extract_to).map_err(|e| e.to_string())?;
                break;
            }
        }

        if !extract_to.exists() {
            return Err("Could not find mta binary inside update archive".to_string());
        }

        self_replace::self_replace(&extract_to).map_err(|e| e.to_string())?;

        if let Ok(exe_path) = std::env::current_exe() {
            let mut current = exe_path.as_path();
            let mut bundle_path: Option<std::path::PathBuf> = None;
            loop {
                if current.extension().and_then(|e| e.to_str()) == Some("app") {
                    bundle_path = Some(current.to_path_buf());
                    break;
                }
                match current.parent() {
                    Some(p) => current = p,
                    None => break,
                }
            }
            if let Some(bundle) = bundle_path {
                let _ = std::process::Command::new("codesign")
                    .args(["-s", "-", "--deep", "--force"])
                    .arg(&bundle)
                    .output();
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let bin_path = tmp_dir.path().join("mta_new");
        let client = reqwest::blocking::Client::builder()
            .user_agent("mta-updater")
            .build()
            .map_err(|e| e.to_string())?;
        let bytes = client
            .get(download_url)
            .send()
            .and_then(|r| r.bytes())
            .map_err(|e| e.to_string())?;
        std::fs::write(&bin_path, &bytes).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&bin_path)
                .map_err(|e| e.to_string())?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&bin_path, perms).map_err(|e| e.to_string())?;
        }

        self_replace::self_replace(&bin_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

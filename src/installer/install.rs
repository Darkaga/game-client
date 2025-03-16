use anyhow::{Context, Result};
use log::{info, warn, error};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task;

use crate::config::Config;
use crate::repository::{GameInfo, GameVersion, FileType, GameFile}; // Added GameFile import
use super::download::{Downloader, DownloadStatus};
use super::version::VersionManager;

/// Installation status
#[derive(Debug, Clone)]
pub enum InstallStatus {
    /// Downloading installer
    Downloading(DownloadStatus),
    /// Installing game
    Installing { game: String, version: String },
    /// Installation completed
    Completed { game: String, install_dir: PathBuf },
    /// Installation failed
    Failed { error: String },
}

/// Game installer (Windows-only implementation)
pub struct Installer {
    /// Configuration
    config: Config,
    /// Downloader
    downloader: Arc<Downloader>,
    /// Version manager
    version_manager: VersionManager,
    /// Progress channel
    progress_tx: Option<mpsc::Sender<InstallStatus>>,
}

impl Installer {
    /// Create a new installer
    pub fn new(config: Config, downloader: Arc<Downloader>) -> Self {
        Self {
            config: config.clone(),
            downloader,
            version_manager: VersionManager::new(),
            progress_tx: None,
        }
    }
    
    /// Set progress channel
    pub fn set_progress_channel(&mut self, tx: mpsc::Sender<InstallStatus>) {
        self.progress_tx = Some(tx);
    }
    
    /// Send installation status
    async fn send_status(&self, status: InstallStatus) {
        if let Some(tx) = &self.progress_tx {
            if let Err(e) = tx.send(status).await {
                warn!("Failed to send installation status: {}", e);
            }
        }
    }
    
    /// Forward download status to installation status
    async fn handle_download_status(&self, status: DownloadStatus) {
        self.send_status(InstallStatus::Downloading(status)).await;
    }
    
    /// Install a game version (Windows-only implementation)
    pub async fn install_version(&self, game: &GameInfo, version: &GameVersion) -> Result<()> {
        info!("Installing {} version {}", game.title, version.name);
        
        // Send installing status
        self.send_status(InstallStatus::Installing {
            game: game.title.clone(),
            version: version.name.clone(),
        }).await;
        
        // Determine the installation directory (this is the game install directory)
        let install_dir = self.config.paths.install_dir.join(&game.id);
        if !install_dir.exists() {
            std::fs::create_dir_all(&install_dir)
                .context("Failed to create installation directory")?;
        }
        
        // Download required files (installers and patches)
        let required_files: Vec<GameFile> = self.version_manager.get_required_files(version)
            .into_iter().cloned().collect();
        let downloaded_paths = self.downloader.download_files(&required_files).await?;
        
        // For each installer file, if its type is Installer, execute it.
        for file in &version.files {
            if file.file_type == FileType::Installer {
                // Find the local path corresponding to the installer file
                let file_path = downloaded_paths.iter()
                    .find(|p| p.ends_with(&file.name))
                    .ok_or_else(|| anyhow::anyhow!("Installer file '{}' not found", file.name))?
                    .clone();
                
                // Run the installer executable (Windows-only)
                let install_result = task::spawn_blocking({
                    let file_path = file_path.clone();
                    move || {
                        Command::new(&file_path)
                            .spawn()
                            .and_then(|mut child| child.wait())
                    }
                }).await??;
                
                if !install_result.success() {
                    self.send_status(InstallStatus::Failed { 
                        error: format!("Installer exited with status: {:?}", install_result) 
                    }).await;
                    return Err(anyhow::anyhow!("Installation failed with status: {:?}", install_result));
                }
            }
        }
        
        // Mark installation complete by writing a marker file in the game install directory
        let install_marker = install_dir.join("installed.txt");
        std::fs::write(&install_marker, format!("Game: {}\nVersion: {}\nInstalled: {}", 
            game.title, version.name, chrono::Local::now()))
            .context("Failed to write installation marker")?;
        
        self.send_status(InstallStatus::Completed {
            game: game.title.clone(),
            install_dir: install_dir.clone(),
        }).await;
        
        info!("Installation completed for {} version {}", game.title, version.name);
        Ok(())
    }
    
    /// Uninstall a game by removing its install directory
    pub fn uninstall_game(&self, game: &GameInfo) -> Result<()> {
        info!("Uninstalling {}", game.title);
        let install_dir = self.config.paths.install_dir.join(&game.id);
        if !install_dir.exists() {
            return Err(anyhow::anyhow!("Game is not installed"));
        }
        std::fs::remove_dir_all(install_dir)
            .context("Failed to remove installation directory")?;
        info!("Uninstallation completed for {}", game.title);
        Ok(())
    }
    
    /// Check if a game is installed (by checking for the marker file)
    pub fn is_installed(&self, game: &GameInfo) -> bool {
        let install_dir = self.config.paths.install_dir.join(&game.id);
        let install_marker = install_dir.join("installed.txt");
        install_marker.exists()
    }
}

impl Clone for Installer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            downloader: self.downloader.clone(),
            version_manager: self.version_manager.clone(),
            progress_tx: self.progress_tx.clone(),
        }
    }
}

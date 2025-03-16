use anyhow::{Context, Result};
use log::{info, warn, error};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::repository::{GameInfo, GameVersion, FileType};
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

/// Game installer
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
    
    /// Install a game version - simplified simulation
    pub async fn install_version(&self, game: &GameInfo, version: &GameVersion) -> Result<()> {
        info!("Installing {} version {}", game.title, version.name);
        
        // Send installing status
        self.send_status(InstallStatus::Installing {
            game: game.title.clone(),
            version: version.name.clone(),
        }).await;
        
        // Simulate installation (wait a bit)
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Get installation directory
        let install_dir = self.config.paths.install_dir.join(&game.id);
        
        // Create installation directory if it doesn't exist
        if !install_dir.exists() {
            std::fs::create_dir_all(&install_dir)
                .context("Failed to create installation directory")?;
        }
        
        // Write a dummy file to simulate installation
        let install_marker = install_dir.join("installed.txt");
        std::fs::write(&install_marker, format!("Game: {}\nVersion: {}\nInstalled: {}", 
            game.title, version.name, chrono::Local::now()))
            .context("Failed to write installation marker")?;
        
        // Send completed status
        self.send_status(InstallStatus::Completed {
            game: game.title.clone(),
            install_dir,
        }).await;
        
        info!("Installation completed for {} version {}", game.title, version.name);
        Ok(())
    }
    
    /// Uninstall a game - simplified simulation
    pub fn uninstall_game(&self, game: &GameInfo) -> Result<()> {
        info!("Uninstalling {}", game.title);
        
        let install_dir = self.config.paths.install_dir.join(&game.id);
        
        if !install_dir.exists() {
            return Err(anyhow::anyhow!("Game is not installed"));
        }
        
        // Remove the directory to simulate uninstallation
        std::fs::remove_dir_all(install_dir)
            .context("Failed to remove installation directory")?;
            
        info!("Uninstallation completed for {}", game.title);
        Ok(())
    }
    
    /// Check if a game is installed
    pub fn is_installed(&self, game: &GameInfo) -> bool {
        let install_dir = self.config.paths.install_dir.join(&game.id);
        let install_marker = install_dir.join("installed.txt");
        install_marker.exists()
    }
}

// For forwarding download status to the UI thread
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
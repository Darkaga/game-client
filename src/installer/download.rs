use anyhow::{Context, Result};
use log::{info, warn, error};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::repository::{GameFile, SmbConnection};

/// Download progress
#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    /// Downloaded size in bytes
    pub downloaded: u64,
    /// Total size in bytes
    pub total: u64,
    /// Progress percentage (0-100)
    pub percentage: f32,
}

/// Download status message
#[derive(Debug, Clone)]
pub enum DownloadStatus {
    /// Download started
    Started { file: String, size: u64 },
    /// Download progress
    Progress(DownloadProgress),
    /// Download completed
    Completed { path: PathBuf },
    /// Download failed
    Failed { error: String },
}

/// Game downloader
pub struct Downloader {
    /// SMB connection
    smb: Arc<SmbConnection>,
    /// Temporary directory
    temp_dir: PathBuf,
    /// Progress channel
    progress_tx: Option<mpsc::Sender<DownloadStatus>>,
}

impl Downloader {
    /// Create a new downloader
    pub fn new(config: &Config, smb: Arc<SmbConnection>) -> Self {
        Self {
            smb,
            temp_dir: config.paths.temp_dir.clone(),
            progress_tx: None,
        }
    }
    
    /// Set progress channel
    pub fn set_progress_channel(&mut self, tx: mpsc::Sender<DownloadStatus>) {
        self.progress_tx = Some(tx);
    }
    
    /// Send download status
    async fn send_status(&self, status: DownloadStatus) {
        if let Some(tx) = &self.progress_tx {
            if let Err(e) = tx.send(status).await {
                warn!("Failed to send download status: {}", e);
            }
        }
    }
    
    /// Download a game file
    pub async fn download_file(&self, file: &GameFile) -> Result<PathBuf> {
        // Create temporary directory if it doesn't exist
        if !self.temp_dir.exists() {
            std::fs::create_dir_all(&self.temp_dir)
                .context("Failed to create temporary directory")?;
        }
        
        let local_path = self.temp_dir.join(&file.name);
        
        // Send started status
        self.send_status(DownloadStatus::Started {
            file: file.name.clone(),
            size: file.size,
        }).await;
        
        // Simulate progress
        for i in 1..=10 {
            let progress = DownloadProgress {
                downloaded: file.size * i / 10,
                total: file.size,
                percentage: (i as f32) * 10.0,
            };
            
            self.send_status(DownloadStatus::Progress(progress)).await;
            
            // Simulate delay
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Download file
        if let Err(e) = self.smb.download_file(&file.remote_path, &local_path).await {
            self.send_status(DownloadStatus::Failed {
                error: e.to_string(),
            }).await;
            return Err(e);
        }
        
        // Send completed status
        self.send_status(DownloadStatus::Completed {
            path: local_path.clone(),
        }).await;
        
        Ok(local_path)
    }
    
    /// Download multiple files
    pub async fn download_files(&self, files: &[GameFile]) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        
        for file in files {
            let path = self.download_file(file).await?;
            paths.push(path);
        }
        
        Ok(paths)
    }
    
    /// Clean up downloaded files
    pub fn cleanup(&self, paths: &[PathBuf]) -> Result<()> {
        for path in paths {
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    warn!("Failed to remove file {}: {}", path.display(), e);
                }
            }
        }
        
        Ok(())
    }
}
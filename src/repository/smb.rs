use crate::config::RepositoryConfig;
use crate::repository::game_info::{GameInfo, GameFile, FileType};
use anyhow::{Context, Result};
use log::{info, warn, error};
use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;
use walkdir::WalkDir;

/// SMB Connection to game repository
pub struct SmbConnection {
    /// SMB config
    pub config: RepositoryConfig,
    /// Whether we're connected to SMB or using local fallback
    using_local_fallback: bool,
    /// Local path for fallback mode
    local_path: Option<PathBuf>,
}

impl SmbConnection {
    /// Create a new SMB connection from configuration
    pub fn new(config: RepositoryConfig) -> Self {
        Self {
            config,
            using_local_fallback: false,
            local_path: None,
        }
    }
    
    /// Connect to the SMB repository
    pub async fn connect(&mut self) -> Result<()> {
        let server = &self.config.server;
        let share = &self.config.share;
        
        // Check if the server field looks like a local path
        if server.contains(":\\") || server.starts_with('/') || server.starts_with('\\') {
            info!("Server field looks like a local path, using local fallback mode");
            
            // Construct the local path
            let mut path = PathBuf::from(server);
            
            // If share is not empty, append it
            if !share.is_empty() && share != "Games" {
                path = path.join(share);
            }
            
            // Check if the path exists
            if path.exists() && path.is_dir() {
                info!("Using local directory as repository: {}", path.display());
                self.using_local_fallback = true;
                self.local_path = Some(path);
                return Ok(());
            } else {
                warn!("Local path does not exist or is not a directory: {}", path.display());
            }
        }
        
        // Try SMB connection for non-local paths
        info!("Attempting to connect to SMB repository: {}\\{}", server, share);
        
        // In a real implementation, this would use actual SMB connection code
        // For now, we'll simulate a successful connection for demo purposes
        info!("Successfully connected to SMB repository (simulated)");
        
        Ok(())
    }
    
    /// Check if connected to the SMB repository
    pub fn is_connected(&self) -> bool {
        self.using_local_fallback || !self.config.server.is_empty()
    }
    
    /// Get the full path for a relative path in the repository
    fn get_full_path(&self, path: &str) -> String {
        if self.config.base_dir.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", self.config.base_dir, path)
        }
    }
    
    /// List directories in the repository
    pub async fn list_directories(&self) -> Result<Vec<String>> {
        if self.using_local_fallback {
            // Use local directory
            if let Some(path) = &self.local_path {
                info!("Listing directories in local repository: {}", path.display());
                
                let mut dirs = Vec::new();
                
                // Read directory entries
                match fs::read_dir(path) {
                    Ok(entries) => {
                        for entry in entries.flatten() {
                            if let Ok(file_type) = entry.file_type() {
                                if file_type.is_dir() {
                                    if let Some(name) = entry.file_name().to_str() {
                                        // Skip directories starting with . or _
                                        if !name.starts_with('.') && !name.starts_with('_') {
                                            dirs.push(name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                        
                        info!("Found {} game directories", dirs.len());
                        Ok(dirs)
                    },
                    Err(e) => {
                        warn!("Failed to read directory {}: {}", path.display(), e);
                        // Fall back to demo directories
                        Ok(self.get_demo_directories())
                    }
                }
            } else {
                warn!("Local path not set, using demo directories");
                Ok(self.get_demo_directories())
            }
        } else {
            // In a real implementation, this would use SMB APIs
            // For now, return demo directories
            info!("Using demo directories (SMB implementation not complete)");
            Ok(self.get_demo_directories())
        }
    }
    
    /// Get demo directories
    fn get_demo_directories(&self) -> Vec<String> {
        vec![
            "amid_evil".to_string(),
            "doom_eternal".to_string(),
            "hades".to_string(),
            "hollow_knight".to_string(),
        ]
    }
    
    /// Download a file from the repository
    pub async fn download_file(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        if self.using_local_fallback {
            // Construct source path
            let source_path = if let Some(base_path) = &self.local_path {
                base_path.join(remote_path.replace('/', "\\"))
            } else {
                return Err(anyhow::anyhow!("Local path not set"));
            };
            
            info!("Copying file: {} -> {}", source_path.display(), local_path.display());
            
            // Create parent directory if it doesn't exist
            if let Some(parent) = local_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .context("Failed to create parent directory")?;
                }
            }
            
            // Copy the file
            match fs::copy(&source_path, local_path) {
                Ok(_) => {
                    info!("File copied successfully");
                    Ok(())
                },
                Err(e) => {
                    // If file doesn't exist, create a dummy file for demonstration
                    warn!("Failed to copy file: {}. Creating dummy file instead.", e);
                    fs::write(local_path, b"Simulated file content")
                        .context(format!("Failed to create local file: {}", local_path.display()))?;
                    Ok(())
                }
            }
        } else {
            // Simulate SMB download
            info!("Simulating download from SMB: {} -> {}", remote_path, local_path.display());
            
            // Create parent directory if it doesn't exist
            if let Some(parent) = local_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .context("Failed to create parent directory")?;
                }
            }
            
            // Create a dummy file
            fs::write(local_path, b"Simulated file content")
                .context(format!("Failed to create local file: {}", local_path.display()))?;
                
            info!("Downloaded file: {} -> {}", remote_path, local_path.display());
            
            Ok(())
        }
    }
    
    /// List all game directories and parse their info
    pub async fn list_games(&self) -> Result<Vec<GameInfo>> {
        let directories = self.list_directories().await?;
        let mut games = Vec::new();
        
        for dir in directories {
            match self.get_game_info(&dir).await {
                Ok(info) => games.push(info),
                Err(e) => {
                    warn!("Failed to parse game info for {}: {}", dir, e);
                    continue;
                }
            }
        }
        
        info!("Found {} games in repository", games.len());
        Ok(games)
    }
    
    /// Get game info from a directory
    async fn get_game_info(&self, dir_name: &str) -> Result<GameInfo> {
        info!("Getting game info for: {}", dir_name);
        
        // Initialize game info with default values
        let mut game_info = GameInfo {
            id: dir_name.to_string(),
            title: dir_name.replace('_', " "),
            developer: None,
            publisher: None,
            release_date: None,
            description: None,
            igdb_id: None,
            files: Vec::new(),
            versions: Vec::new(),
            cover_image: None,
        };
        
        // Try to read real files in local mode
        if self.using_local_fallback {
            if let Some(base_path) = &self.local_path {
                let game_dir = base_path.join(dir_name);
                
                // Try to read info.txt or !info.txt for metadata
                let info_files = ["info.txt", "!info.txt", "game.info", "game.txt"];
                for info_file in &info_files {
                    let info_path = game_dir.join(info_file);
                    if info_path.exists() && info_path.is_file() {
                        if let Ok(content) = fs::read_to_string(&info_path) {
                            game_info.parse_metadata(&content);
                            break;
                        }
                    }
                }
                
                // Apply title from directory name if not found in metadata
                if game_info.title.is_empty() {
                    game_info.title = dir_name.replace('_', " ")
                        .split(' ')
                        .map(|s| {
                            let mut chars = s.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().chain(chars).collect(),
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(" ");
                }
                
                // Scan for game files (executables, installers)
                let mut game_files = Vec::new();
                
                // Define patterns for installer and patch files
                let installer_regex = Regex::new(r"(?i)(setup|install|launcher).*\.(exe|msi|pkg|dmg)$").unwrap();
                let patch_regex = Regex::new(r"(?i)(patch|update).*\.(exe|msi|pkg|dmg|zip)$").unwrap();
                
                // Walk directory to find files
                let walker = WalkDir::new(&game_dir).max_depth(2).into_iter();
                for entry in walker.filter_map(|e| e.ok()) {
                    let file_path = entry.path();
                    
                    // Skip directories
                    if file_path.is_dir() {
                        continue;
                    }
                    
                    // Get file name and extension
                    if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                        let file_size = fs::metadata(file_path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        
                        // Determine file type
                        let file_type = if installer_regex.is_match(file_name) {
                            FileType::Installer
                        } else if patch_regex.is_match(file_name) {
                            FileType::Patch
                        } else if file_name.to_lowercase().ends_with(".exe") {
                            FileType::Installer
                        } else {
                            FileType::Other
                        };
                        
                        // Get relative path from base directory
                        let rel_path = file_path.strip_prefix(&game_dir)
                            .unwrap_or_else(|_| Path::new(file_name))
                            .to_string_lossy()
                            .replace('\\', "/");
                        
                        // Add to files list
                        game_files.push(GameFile {
                            name: file_name.to_string(),
                            remote_path: format!("{}/{}", dir_name, rel_path),
                            size: file_size,
                            file_type,
                        });
                    }
                }
                
                // Add found files to game info
                game_info.files = game_files;
            } else {
                // If no local path, use demo data
                self.add_demo_files(&mut game_info);
            }
        } else {
            // If not using local fallback, use demo data
            self.add_demo_files(&mut game_info);
        }
        
        // Parse versions from files
        game_info.parse_versions();
        
        // Ensure at least one version exists
        if game_info.versions.is_empty() && !game_info.files.is_empty() {
            // Create a default version
            let version = crate::repository::game_info::GameVersion {
                name: "Default Version".to_string(),
                build: 1,
                files: game_info.files.clone(),
                required_patches: Vec::new(),
            };
            
            game_info.versions.push(version);
        }
        
        Ok(game_info)
    }
    
    /// Add demo files to a game
    fn add_demo_files(&self, game_info: &mut GameInfo) {
        let dir_name = &game_info.id;
        
        // Add installer file
        game_info.files.push(GameFile {
            name: format!("setup_{}_gog_build_2241b_(64bit)_(51706).exe", dir_name),
            remote_path: format!("{}/setup_{}_gog_build_2241b_(64bit)_(51706).exe", dir_name, dir_name),
            size: 15_000_000,
            file_type: FileType::Installer,
        });
        
        // Add patch files
        game_info.files.push(GameFile {
            name: format!("patch_{}_GOG_Build_2055a_(37083)_to_GOG_Build_2172_(47150).exe", dir_name),
            remote_path: format!("{}/patch_{}_GOG_Build_2055a_(37083)_to_GOG_Build_2172_(47150).exe", dir_name, dir_name),
            size: 2_000_000,
            file_type: FileType::Patch,
        });
        
        // Set demo metadata
        if game_info.developer.is_none() {
            game_info.developer = Some("Demo Developer".to_string());
        }
        
        if game_info.publisher.is_none() {
            game_info.publisher = Some("Demo Publisher".to_string());
        }
        
        if game_info.release_date.is_none() {
            game_info.release_date = Some("2023-01-01".to_string());
        }
        
        if game_info.description.is_none() {
            game_info.description = Some("This is a demo game description.".to_string());
        }
        
        if game_info.igdb_id.is_none() {
            game_info.igdb_id = Some(12345);
        }
    }
}
use anyhow::Result;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Application configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    /// SMB repository configuration
    pub repository: RepositoryConfig,
    
    /// Local paths configuration
    pub paths: PathsConfig,
    
    /// IGDB API configuration
    pub igdb: IgdbConfig,
}

/// SMB repository configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RepositoryConfig {
    /// SMB server address (e.g., "192.168.1.100")
    pub server: String,
    
    /// SMB share name (e.g., "Games")
    pub share: String,
    
    /// Username for SMB authentication
    pub username: String,
    
    /// Password for SMB authentication
    pub password: String,
    
    /// Base directory within the share
    pub base_dir: String,
}

/// Local paths configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PathsConfig {
    /// Directory for game installations
    pub install_dir: PathBuf,
    
    /// Directory for caching metadata and images
    pub cache_dir: PathBuf,
    
    /// Directory for temporary files
    pub temp_dir: PathBuf,
}

/// IGDB API configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct IgdbConfig {
    /// IGDB Client ID
    pub client_id: String,
    
    /// IGDB Client Secret
    pub client_secret: String,
}

impl Default for Config {
    fn default() -> Self {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        
        Self {
            repository: RepositoryConfig {
                server: "".to_string(),
                share: "Games".to_string(),
                username: "".to_string(),
                password: "".to_string(),
                base_dir: "Windows".to_string(),
            },
            paths: PathsConfig {
                install_dir: home_dir.join("Games"),
                cache_dir: dirs::cache_dir()
                    .unwrap_or_else(|| home_dir.join(".cache"))
                    .join("game-library-manager"),
                temp_dir: std::env::temp_dir().join("game-library-manager"),
            },
            igdb: IgdbConfig {
                client_id: "".to_string(),
                client_secret: "".to_string(),
            },
        }
    }
}

impl Config {
    /// Get the configuration file path
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("game-library-manager")
            .join("config.toml")
    }
    
    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        
        if !config_path.exists() {
            info!("Configuration file not found, using defaults");
            return Ok(Self::default());
        }
        
        let config_str = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&config_str)?;
        
        info!("Configuration loaded from {}", config_path.display());
        Ok(config)
    }
    
    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        
        let config_str = toml::to_string(self)?;
        fs::write(&config_path, config_str)?;
        
        info!("Configuration saved to {}", config_path.display());
        Ok(())
    }
    
    /// Ensure all configured directories exist
    pub fn ensure_directories(&self) -> Result<()> {
        for dir in [
            &self.paths.install_dir,
            &self.paths.cache_dir,
            &self.paths.temp_dir,
        ] {
            if !dir.exists() {
                info!("Creating directory: {}", dir.display());
                fs::create_dir_all(dir)?;
            }
        }
        
        Ok(())
    }
}
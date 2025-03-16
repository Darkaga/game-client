use anyhow::{Context, Result};
use log::{info, warn, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use super::igdb::IgdbGame;

/// Metadata cache for storing and retrieving metadata
#[derive(Clone)]
pub struct MetadataCache {
    /// Base directory for cache
    cache_dir: PathBuf,
    /// Loaded metadata
    metadata: HashMap<String, CachedMetadata>,
}

/// Cached metadata entry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CachedMetadata {
    /// Game ID (directory name)
    pub game_id: String,
    /// IGDB ID
    pub igdb_id: Option<u32>,
    /// IGDB metadata
    pub igdb_data: Option<IgdbGame>,
    /// Cover image path (relative to cache directory)
    pub cover_path: Option<String>,
    /// Last update timestamp
    pub last_updated: u64,
}

impl MetadataCache {
    /// Create a new metadata cache
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        // Create cache directories if they don't exist
        let metadata_dir = cache_dir.join("metadata");
        let images_dir = cache_dir.join("images");
        
        for dir in [&metadata_dir, &images_dir] {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }
        }
        
        let cache = Self {
            cache_dir,
            metadata: HashMap::new(),
        };
        
        Ok(cache)
    }
    
    /// Get metadata directory
    pub fn metadata_dir(&self) -> PathBuf {
        self.cache_dir.join("metadata")
    }
    
    /// Get images directory
    pub fn images_dir(&self) -> PathBuf {
        self.cache_dir.join("images")
    }
    
    /// Get the path to a metadata file
    fn get_metadata_path(&self, game_id: &str) -> PathBuf {
        self.metadata_dir().join(format!("{}.json", game_id))
    }
    
    /// Load all cached metadata
    pub fn load_all(&mut self) -> Result<()> {
        let metadata_dir = self.metadata_dir();
        
        if !metadata_dir.exists() {
            fs::create_dir_all(&metadata_dir)?;
            return Ok(());
        }
        
        info!("Loading cached metadata from {}", metadata_dir.display());
        
        // Walk metadata directory
        let entries = fs::read_dir(&metadata_dir)
            .with_context(|| format!("Failed to read metadata directory: {}", metadata_dir.display()))?;
        
        let mut loaded = 0;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            // Skip non-JSON files
            if path.extension().map_or(true, |ext| ext != "json") {
                continue;
            }
            
            // Get game ID from filename
            let game_id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid metadata file name: {}", path.display()))?;
            
            // Load metadata
            match self.load_metadata_file(&path) {
                Ok(metadata) => {
                    self.metadata.insert(game_id.to_string(), metadata);
                    loaded += 1;
                }
                Err(e) => {
                    warn!("Failed to load metadata for game {}: {}", game_id, e);
                }
            }
        }
        
        info!("Loaded metadata for {} games", loaded);
        Ok(())
    }
    
    /// Load metadata from a file
    fn load_metadata_file(&self, path: &Path) -> Result<CachedMetadata> {
        let json_str = fs::read_to_string(path)
            .with_context(|| format!("Failed to read metadata file: {}", path.display()))?;
        
        let metadata: CachedMetadata = serde_json::from_str(&json_str)
            .with_context(|| format!("Failed to parse metadata file: {}", path.display()))?;
        
        Ok(metadata)
    }
    
    /// Load metadata for a specific game
    pub fn load_metadata(&mut self, game_id: &str) -> Result<CachedMetadata> {
        // Check if metadata is already loaded
        if let Some(metadata) = self.metadata.get(game_id) {
            return Ok(metadata.clone());
        }
        
        let path = self.get_metadata_path(game_id);
        
        if path.exists() {
            let metadata = self.load_metadata_file(&path)?;
            self.metadata.insert(game_id.to_string(), metadata.clone());
            return Ok(metadata);
        }
        
        // Create new metadata if it doesn't exist
        let metadata = self.create_metadata(game_id);
        self.metadata.insert(game_id.to_string(), metadata.clone());
        
        Ok(metadata)
    }
    
    /// Save metadata for a specific game
    pub fn save_metadata(&mut self, metadata: CachedMetadata) -> Result<()> {
        let game_id = metadata.game_id.clone();
        
        // Update in-memory cache
        self.metadata.insert(game_id.clone(), metadata.clone());
        
        // Save to file
        let path = self.get_metadata_path(&game_id);
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        
        let json_str = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize metadata")?;
        
        fs::write(&path, json_str)
            .with_context(|| format!("Failed to write metadata file: {}", path.display()))?;
        
        info!("Saved metadata for game {}", game_id);
        Ok(())
    }
    
    /// Get metadata for a specific game
    pub fn get_metadata(&self, game_id: &str) -> Option<&CachedMetadata> {
        self.metadata.get(game_id)
    }
    
    /// Get metadata for a specific game (mutable)
    pub fn get_metadata_mut(&mut self, game_id: &str) -> Option<&mut CachedMetadata> {
        self.metadata.get_mut(game_id)
    }
    
    /// Check if metadata exists for a specific game
    pub fn has_metadata(&self, game_id: &str) -> bool {
        self.metadata.contains_key(game_id)
    }
    
    /// Get path for a cached cover image
    pub fn get_cover_path(&self, game_id: &str) -> PathBuf {
        self.images_dir().join(format!("{}_cover.jpg", game_id))
    }
    
    /// Check if a cover image exists
    pub fn has_cover(&self, game_id: &str) -> bool {
        self.get_cover_path(game_id).exists()
    }
    
    /// Create a new metadata entry
    pub fn create_metadata(&self, game_id: &str) -> CachedMetadata {
        CachedMetadata {
            game_id: game_id.to_string(),
            igdb_id: None,
            igdb_data: None,
            cover_path: None,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Update metadata with IGDB data
    pub fn update_with_igdb(&mut self, game_id: &str, igdb_game: IgdbGame) -> Result<()> {
        // Load existing metadata or create new
        let mut metadata = if self.has_metadata(game_id) {
            self.get_metadata(game_id)
                .cloned()
                .unwrap()
        } else {
            self.create_metadata(game_id)
        };
        
        // Update fields
        metadata.igdb_id = Some(igdb_game.id);
        metadata.igdb_data = Some(igdb_game);
        metadata.last_updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Save updated metadata
        self.save_metadata(metadata)
    }
    
    /// Update cover path in metadata
    pub fn update_cover_path(&mut self, game_id: &str, relative_path: &str) -> Result<()> {
        if let Some(metadata) = self.get_metadata_mut(game_id) {
            metadata.cover_path = Some(relative_path.to_string());
            metadata.last_updated = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            // Save updated metadata
            let metadata_clone = metadata.clone();
            self.save_metadata(metadata_clone)?;
        }
        
        Ok(())
    }
    
    /// Check if metadata is stale (older than specified days)
    pub fn is_stale(&self, game_id: &str, days: u64) -> bool {
        if let Some(metadata) = self.get_metadata(game_id) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            let age_seconds = now.saturating_sub(metadata.last_updated);
            let age_days = age_seconds / 86400; // 86400 seconds in a day
            
            age_days > days
        } else {
            true
        }
    }
}
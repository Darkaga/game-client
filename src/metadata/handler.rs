use anyhow::Result;
use log::{info, warn, error};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::config::IgdbConfig;
use super::igdb::{IgdbClient, IgdbGame};
use super::cache::{MetadataCache, CachedMetadata};

/// Metadata operation status
#[derive(Debug, Clone)]
pub enum MetadataStatus {
    /// Started fetching metadata
    Started { game_id: String, game_name: String },
    /// Successfully fetched metadata
    Success { game_id: String, game_name: String },
    /// Failed to fetch metadata
    Failed { game_id: String, game_name: String, error: String },
    /// Progress update
    Progress { completed: usize, total: usize },
    /// Operation completed
    Completed { successful: usize, failed: usize, total: usize },
}

/// Metadata handler for managing game metadata
#[derive(Clone)]
pub struct MetadataHandler {
    igdb_client: IgdbClient,
    cache: MetadataCache,
    progress_tx: Option<Sender<MetadataStatus>>,
    last_refresh: std::collections::HashMap<String, Instant>,
}

impl MetadataHandler {
    /// Create a new metadata handler
    pub fn new(igdb_config: IgdbConfig, cache_dir: PathBuf) -> Result<Self> {
        let igdb_client = IgdbClient::new(igdb_config);
        let cache = MetadataCache::new(cache_dir)?;
        
        Ok(Self {
            igdb_client,
            cache,
            progress_tx: None,
            last_refresh: std::collections::HashMap::new(),
        })
    }
    
    /// Set progress channel
    pub fn set_progress_channel(&mut self, tx: Sender<MetadataStatus>) {
        self.progress_tx = Some(tx);
    }
    
    /// Send status update
    fn send_status(&self, status: MetadataStatus) {
        if let Some(tx) = &self.progress_tx {
            if let Err(e) = tx.send(status) {
                warn!("Failed to send metadata status: {}", e);
            }
        }
    }
    
    /// Initialize the metadata handler
    pub async fn initialize(&mut self) -> Result<()> {
        // Load cached metadata
        self.cache.load_all()?;
        
        // Try to authenticate with IGDB if credentials are configured
        if self.igdb_client.is_configured() {
            match self.igdb_client.authenticate().await {
                Ok(_) => info!("Successfully authenticated with IGDB"),
                Err(e) => warn!("Failed to authenticate with IGDB: {}", e),
            }
        } else {
            warn!("IGDB credentials not configured");
        }
        
        Ok(())
    }
    
    /// Get metadata for a game
    pub fn get_metadata(&self, game_id: &str) -> Option<&CachedMetadata> {
        self.cache.get_metadata(game_id)
    }
    
    /// Check if a game has IGDB metadata
    pub fn has_igdb_metadata(&self, game_id: &str) -> bool {
        if let Some(metadata) = self.cache.get_metadata(game_id) {
            metadata.igdb_data.is_some()
        } else {
            false
        }
    }
    
    /// Check if a game has a cover image
    pub fn has_cover(&self, game_id: &str) -> bool {
        self.cache.has_cover(game_id)
    }
    
    /// Get cover image path
    pub fn get_cover_path(&self, game_id: &str) -> PathBuf {
        self.cache.get_cover_path(game_id)
    }
    
    /// Search IGDB for a game by name
    pub async fn search_game(&mut self, name: &str) -> Result<Vec<IgdbGame>> {
        self.igdb_client.search_game(name).await
    }
    
    /// Find best match for a game name
    pub async fn find_best_match(&mut self, name: &str) -> Result<Option<IgdbGame>> {
        self.igdb_client.find_best_match(name).await
    }
    
    /// Fetch metadata for a game and update cache
    pub async fn fetch_and_cache_metadata(&mut self, game_id: &str, game_name: &str) -> Result<bool> {
        // Send started status
        self.send_status(MetadataStatus::Started {
            game_id: game_id.to_string(),
            game_name: game_name.to_string(),
        });
        
        // Check if metadata is already cached and not stale
        if self.has_igdb_metadata(game_id) && !self.cache.is_stale(game_id, 30) {
            info!("Using cached metadata for game {}", game_id);
            
            // Update last refresh time
            self.last_refresh.insert(game_id.to_string(), Instant::now());
            
            // Send success status
            self.send_status(MetadataStatus::Success {
                game_id: game_id.to_string(),
                game_name: game_name.to_string(),
            });
            
            return Ok(true);
        }
        
        info!("Fetching metadata for game: {} ({})", game_id, game_name);
        
        // Find best match on IGDB
        let igdb_game = match self.find_best_match(game_name).await {
            Ok(Some(game)) => game,
            Ok(None) => {
                warn!("No IGDB match found for game: {}", game_name);
                
                // Send failed status
                self.send_status(MetadataStatus::Failed {
                    game_id: game_id.to_string(),
                    game_name: game_name.to_string(),
                    error: "No matching game found on IGDB".to_string(),
                });
                
                return Ok(false);
            }
            Err(e) => {
                error!("IGDB search error for game {}: {}", game_name, e);
                
                // Send failed status
                self.send_status(MetadataStatus::Failed {
                    game_id: game_id.to_string(),
                    game_name: game_name.to_string(),
                    error: format!("IGDB API error: {}", e),
                });
                
                return Err(e);
            }
        };
        
        info!("Found IGDB match for {}: {} (ID: {})", 
            game_name, igdb_game.name, igdb_game.id);
        
        // Update cache with IGDB data
        self.cache.update_with_igdb(game_id, igdb_game)?;
        
        // Update last refresh time
        self.last_refresh.insert(game_id.to_string(), Instant::now());
        
        // Send success status
        self.send_status(MetadataStatus::Success {
            game_id: game_id.to_string(),
            game_name: game_name.to_string(),
        });
        
        Ok(true)
    }
    
    /// Download and cache cover image
    pub async fn download_cover(&mut self, game_id: &str, size: &str) -> Result<bool> {
        // We need to get the cover image ID first before borrowing self.igdb_client
        let cover_image_id: Option<String> = {
            // This scope ensures the borrow of self for get_metadata() ends
            match self.get_metadata(game_id) {
                Some(metadata) => match &metadata.igdb_data {
                    Some(igdb_data) => match &igdb_data.cover {
                        Some(cover) => Some(cover.image_id.clone()),
                        None => None,
                    },
                    None => None,
                },
                None => None,
            }
        };
        
        let cover_image_id = match cover_image_id {
            Some(id) => id,
            None => return Ok(false),
        };
        
        // Get cover path
        let cover_path = self.cache.get_cover_path(game_id);
        
        // Check if cover already exists
        if cover_path.exists() {
            return Ok(true);
        }
        
        info!("Downloading cover for game {}", game_id);
        
        // Now we can borrow self.igdb_client
        match self.igdb_client.download_cover(&cover_image_id, size, &cover_path).await {
            Ok(_) => {
                // Update metadata with cover path
                let relative_path = format!("images/{}_cover.jpg", game_id);
                self.cache.update_cover_path(game_id, &relative_path)?;
                Ok(true)
            },
            Err(e) => {
                error!("Failed to download cover for game {}: {}", game_id, e);
                Ok(false)
            }
        }
    }
    
    /// Refresh metadata for a game
    pub async fn refresh_metadata(&mut self, game_id: &str, game_name: &str) -> Result<bool> {
        // Force refresh by fetching new data
        info!("Refreshing metadata for game: {} ({})", game_id, game_name);
        
        let result = self.fetch_and_cache_metadata(game_id, game_name).await?;
        
        if result {
            // Also refresh cover if we have metadata
            if self.has_igdb_metadata(game_id) {
                self.download_cover(game_id, "cover_big").await?;
            }
        }
        
        Ok(result)
    }
    
    /// Update metadata for all games in the library
    pub async fn update_library_metadata(
        &mut self,
        games: &[(String, String)], // (game_id, game_name) pairs
    ) -> Result<()> {
        let total = games.len();
        let mut updated = 0;
        let mut failed = 0;
        
        info!("Updating metadata for {} games", total);
        
        // Send initial progress
        self.send_status(MetadataStatus::Progress {
            completed: 0,
            total,
        });
        
        for (i, (game_id, game_name)) in games.iter().enumerate() {
            info!("Processing game {}/{}: {}", i + 1, total, game_name);
            
            // Send status update
            self.send_status(MetadataStatus::Started {
                game_id: game_id.to_string(),
                game_name: game_name.to_string(),
            });
            
            // Only update if metadata is missing or stale
            if !self.has_igdb_metadata(game_id) || self.cache.is_stale(game_id, 30) {
                match self.fetch_and_cache_metadata(game_id, game_name).await {
                    Ok(true) => {
                        // Download cover if metadata was fetched
                        let _ = self.download_cover(game_id, "cover_big").await;
                        updated += 1;
                        
                        // Send success status
                        self.send_status(MetadataStatus::Success {
                            game_id: game_id.to_string(),
                            game_name: game_name.to_string(),
                        });
                    }
                    Ok(false) => {
                        failed += 1;
                        
                        // Send failure status
                        self.send_status(MetadataStatus::Failed {
                            game_id: game_id.to_string(),
                            game_name: game_name.to_string(),
                            error: "Could not find metadata".to_string(),
                        });
                    }
                    Err(e) => {
                        error!("Error updating metadata for game {}: {}", game_name, e);
                        failed += 1;
                        
                        // Send failure status
                        self.send_status(MetadataStatus::Failed {
                            game_id: game_id.to_string(),
                            game_name: game_name.to_string(),
                            error: e.to_string(),
                        });
                    }
                }
            } else {
                // Already up to date
                self.send_status(MetadataStatus::Success {
                    game_id: game_id.to_string(),
                    game_name: game_name.to_string(),
                });
            }
            
            // Send progress update
            self.send_status(MetadataStatus::Progress {
                completed: i + 1,
                total,
            });
        }
        
        // Send completion status
        self.send_status(MetadataStatus::Completed {
            successful: updated,
            failed,
            total,
        });
        
        info!("Updated metadata for {}/{} games ({} failed)", updated, total, failed);
        Ok(())
    }
    
    /// Batch update metadata for multiple games
    pub async fn batch_update_metadata(&mut self, games: &[(&str, &str)]) -> Result<()> {
        let total = games.len();
        let mut updated = 0;
        let mut failed = 0;
        
        info!("Starting batch metadata update for {} games", total);
        
        // Convert to owned String pairs
        let game_pairs: Vec<(String, String)> = games
            .iter()
            .map(|(id, name)| (id.to_string(), name.to_string()))
            .collect();
        
        // Use the existing update_library_metadata method
        self.update_library_metadata(&game_pairs).await?;
        
        Ok(())
    }
    
    /// Check if a game was recently refreshed
    pub fn was_recently_refreshed(&self, game_id: &str, seconds: u64) -> bool {
        if let Some(last_time) = self.last_refresh.get(game_id) {
            let elapsed = last_time.elapsed();
            elapsed < Duration::from_secs(seconds)
        } else {
            false
        }
    }
}
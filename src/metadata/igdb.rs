use anyhow::{Context, Result};
use log::{info, warn, error};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use crate::config::IgdbConfig;

/// IGDB game information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IgdbGame {
    pub id: u32,
    pub name: String,
    pub summary: Option<String>,
    pub storyline: Option<String>,
    pub first_release_date: Option<u64>,
    pub cover: Option<IgdbCover>,
    pub involved_companies: Option<Vec<IgdbCompany>>,
    pub genres: Option<Vec<IgdbGenre>>,
    pub platforms: Option<Vec<IgdbPlatform>>,
    pub slug: Option<String>,
    pub url: Option<String>,
    pub total_rating: Option<f32>,
    pub total_rating_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IgdbCover {
    pub id: u32,
    pub url: Option<String>,
    pub image_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IgdbCompany {
    pub id: u32,
    pub company: IgdbCompanyInfo,
    pub developer: bool,
    pub publisher: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IgdbCompanyInfo {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IgdbGenre {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IgdbPlatform {
    pub id: u32,
    pub name: String,
    pub slug: Option<String>,
}

/// Twitch OAuth token response
#[derive(Debug, Deserialize)]
struct TwitchAuthResponse {
    access_token: String,
    expires_in: u64,
}

/// IGDB API client
#[derive(Clone)]
pub struct IgdbClient {
    config: IgdbConfig,
    client: Client,
    access_token: Option<String>,
    token_expiry: Option<Instant>,
    base_url: String,
}

impl IgdbClient {
    /// Create a new IGDB client
    pub fn new(config: IgdbConfig) -> Self {
        Self {
            config,
            client: Client::new(),
            access_token: None,
            token_expiry: None,
            base_url: String::from("https://api.igdb.com/v4"),
        }
    }
    
    /// Check if client ID and secret are configured
    pub fn is_configured(&self) -> bool {
        !self.config.client_id.is_empty() && !self.config.client_secret.is_empty()
    }
    
    /// Check if authentication is needed
    fn needs_authentication(&self) -> bool {
        match (self.access_token.as_ref(), self.token_expiry) {
            (Some(_), Some(expiry)) => {
                // Refresh token if it will expire in less than 5 minutes
                expiry <= Instant::now() + Duration::from_secs(300)
            }
            _ => true,
        }
    }
    
    /// Authenticate with Twitch API to get access token
    pub async fn authenticate(&mut self) -> Result<()> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("IGDB credentials not configured"));
        }
        
        if !self.needs_authentication() {
            return Ok(());
        }
        
        info!("Authenticating with Twitch API for IGDB access");
        
        // Twitch OAuth endpoint
        let url = "https://id.twitch.tv/oauth2/token";
        
        // Request parameters
        let params = [
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.as_str()),
            ("grant_type", "client_credentials"),
        ];
        
        // Send POST request
        let response = self.client
            .post(url)
            .form(&params)
            .send()
            .await
            .context("Failed to send authentication request")?;
        
        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Authentication failed: {} - {}", status, text));
        }
        
        // Parse response
        let auth: TwitchAuthResponse = response
            .json()
            .await
            .context("Failed to parse authentication response")?;
        
        // Store token and expiry
        self.access_token = Some(auth.access_token);
        self.token_expiry = Some(Instant::now() + Duration::from_secs(auth.expires_in));
        
        info!("Successfully authenticated with Twitch API");
        Ok(())
    }
    
    /// Ensure client is authenticated before making a request
    async fn ensure_authenticated(&mut self) -> Result<()> {
        if self.needs_authentication() {
            self.authenticate().await?;
        }
        Ok(())
    }
    
    /// Create authorization headers for IGDB requests
    fn create_headers(&self) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        
        // Add Client-ID header
        headers.insert(
            "Client-ID",
            header::HeaderValue::from_str(&self.config.client_id)
                .context("Invalid client ID")?,
        );
        
        // Add Authorization header
        if let Some(token) = &self.access_token {
            headers.insert(
                header::AUTHORIZATION,
                header::HeaderValue::from_str(&format!("Bearer {}", token))
                    .context("Invalid access token")?,
            );
        } else {
            return Err(anyhow::anyhow!("No access token available"));
        }
        
        // Add Content-Type header
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        
        Ok(headers)
    }
    
    /// Execute a query against the IGDB API
    async fn execute_query<T: for<'de> Deserialize<'de>>(
        &mut self,
        endpoint: &str,
        query: &str,
    ) -> Result<Vec<T>> {
        // Ensure we're authenticated
        self.ensure_authenticated().await?;
        
        // Build request URL
        let url = format!("{}/{}", self.base_url, endpoint);
        
        // Create headers
        let headers = self.create_headers()?;
        
        // Send request
        let response = self.client
            .post(&url)
            .headers(headers)
            .body(query.to_string())
            .send()
            .await
            .context(format!("Failed to send request to {}", endpoint))?;
        
        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("IGDB API error: {} - {}", status, text));
        }
        
        // Parse response
        let results: Vec<T> = response
            .json()
            .await
            .context("Failed to parse IGDB response")?;
        
        Ok(results)
    }
    
    /// Search for a game by name
    pub async fn search_game(&mut self, name: &str) -> Result<Vec<IgdbGame>> {
        info!("Searching for game: {}", name);
        
        // Build IGDB query
        // This query includes all fields we want to retrieve
        let query = format!(
            r#"search "{}";
            fields id,name,summary,storyline,first_release_date,
            cover.image_id,
            involved_companies.company.name,involved_companies.developer,involved_companies.publisher,
            genres.name,
            platforms.name,platforms.slug,
            slug,url,total_rating,total_rating_count;
            limit 10;"#,
            name
        );
        
        // Execute query
        let games = self.execute_query::<IgdbGame>("games", &query).await?;
        
        info!("Found {} games matching '{}'", games.len(), name);
        
        Ok(games)
    }
    
    /// Get a game by ID
    pub async fn get_game(&mut self, id: u32) -> Result<Option<IgdbGame>> {
        info!("Getting game with ID: {}", id);
        
        // Build IGDB query
        let query = format!(
            r#"where id = {};
            fields id,name,summary,storyline,first_release_date,
            cover.image_id,
            involved_companies.company.name,involved_companies.developer,involved_companies.publisher,
            genres.name,
            platforms.name,platforms.slug,
            slug,url,total_rating,total_rating_count;
            limit 1;"#,
            id
        );
        
        // Execute query
        let mut games = self.execute_query::<IgdbGame>("games", &query).await?;
        
        Ok(games.pop())
    }
    
    /// Get cover URL for a game
    pub fn get_cover_url(&self, image_id: &str, size: &str) -> String {
        format!("https://images.igdb.com/igdb/image/upload/t_{}/{}.jpg", size, image_id)
    }
    
    /// Download cover image
    pub async fn download_cover(&mut self, image_id: &str, size: &str, path: &std::path::Path) -> Result<()> {
        info!("Downloading cover image {} to {}", image_id, path.display());
        
        // Get image URL
        let url = self.get_cover_url(image_id, size);
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create parent directory")?;
            }
        }
        
        // Download image
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to download cover image")?;
        
        // Check response status
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to download cover image: {}", response.status()));
        }
        
        // Get image bytes
        let bytes = response
            .bytes()
            .await
            .context("Failed to read cover image data")?;
        
        // Write image to file
        std::fs::write(path, bytes)
            .context("Failed to write image file")?;
            
        info!("Cover image successfully downloaded to {}", path.display());
        Ok(())
    }
    
    /// Helper method to find the best match for a game name
    pub async fn find_best_match(&mut self, name: &str) -> Result<Option<IgdbGame>> {
        // Search for games
        let games = self.search_game(name).await?;
        
        if games.is_empty() {
            return Ok(None);
        }
        
        // Start with exact match
        for game in &games {
            if game.name.to_lowercase() == name.to_lowercase() {
                return Ok(Some(game.clone()));
            }
        }
        
        // Otherwise, return the first result
        Ok(Some(games[0].clone()))
    }
}
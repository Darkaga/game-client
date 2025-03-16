use anyhow::Result;
use std::path::PathBuf;
use tokio::runtime::Runtime;
use crate::config::IgdbConfig;
use crate::metadata::handler::MetadataHandler;

/// Test function to verify IGDB API integration
pub async fn test_igdb_api(client_id: &str, client_secret: &str, cache_dir: PathBuf) -> Result<()> {
    println!("Testing IGDB API integration...");
    
    // Create config
    let config = IgdbConfig {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    };
    
    // Create handler
    let mut handler = MetadataHandler::new(config.clone(), cache_dir)?;
    
    // Initialize (authenticates with IGDB)
    println!("Authenticating with IGDB...");
    handler.initialize().await?;
    println!("Authentication successful");
    
    // Test game search
    let test_game = "The Witcher 3";
    println!("Searching for game: {}", test_game);
    
    let results = handler.search_game(test_game).await?;
    println!("Found {} results", results.len());
    
    for (i, game) in results.iter().enumerate() {
        println!("Result {}: {} (ID: {})", i + 1, game.name, game.id);
        println!("  Released: {}", game.first_release_date
            .map(|ts| {
                let dt = chrono::NaiveDateTime::from_timestamp(ts as i64, 0);
                dt.format("%Y-%m-%d").to_string()
            })
            .unwrap_or_else(|| "Unknown".to_string()));
        
        if let Some(summary) = &game.summary {
            let short_summary = if summary.len() > 100 {
                format!("{}...", &summary[..100])
            } else {
                summary.clone()
            };
            println!("  Summary: {}", short_summary);
        }
        
        if let Some(cover) = &game.cover {
            println!("  Cover Image ID: {}", cover.image_id);
        }
        
        println!();
    }
    
    // Test cover download if we have results
    if !results.is_empty() && results[0].cover.is_some() {
        let game = &results[0];
        let cover = game.cover.as_ref().unwrap();
        let image_id = &cover.image_id;
        
        println!("Testing cover download for '{}' (image_id: {})", game.name, image_id);
        
        let temp_dir = std::env::temp_dir().join("igdb_test");
        std::fs::create_dir_all(&temp_dir)?;
        
        let image_path = temp_dir.join(format!("{}_cover.jpg", game.id));
        
        println!("Downloading cover to {}", image_path.display());
        
        // Create a new IGDB client directly for cover download
        let mut igdb_client = crate::metadata::igdb::IgdbClient::new(config);
        igdb_client.authenticate().await?;
        igdb_client.download_cover(image_id, "cover_big", &image_path).await?;
        
        println!("Cover downloaded successfully");
        println!("Image saved to: {}", image_path.display());
    }
    
    println!("IGDB API test completed successfully");
    Ok(())
}

/// Run the IGDB API test with a runtime
pub fn run_igdb_test(client_id: &str, client_secret: &str, cache_dir: PathBuf) -> Result<()> {
    // Create a runtime
    let rt = Runtime::new()?;
    
    // Run the test
    rt.block_on(test_igdb_api(client_id, client_secret, cache_dir))
}
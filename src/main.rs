mod config;
mod repository;
mod metadata;
mod installer;
mod ui;

use anyhow::Result;
use eframe::NativeOptions;
use log::{info, LevelFilter};
use std::path::PathBuf;

use config::Config;
use ui::app::GameLibraryApp;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .init();
    
    info!("Starting Game Library Manager");
    
    // Load configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            // Create default configuration if it doesn't exist
            let config = Config::default();
            config.save()?;
            config
        }
    };
    
    // GUI Options
    let options = NativeOptions {
        initial_window_size: Some(egui::vec2(1280.0, 800.0)),
        icon_data: None, // TODO: Add application icon
        ..Default::default()
    };
    
    // Run application
    eframe::run_native(
        "Game Library Manager",
        options,
        Box::new(|cc| Box::new(GameLibraryApp::new(cc, config))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run application: {}", e))
}
use eframe::egui;
use egui::{Align, Layout, RichText, Ui};
use std::path::PathBuf;
use std::fs;
use image;

use crate::repository::GameInfo;
use crate::metadata::handler::MetadataHandler;

/// Game action
pub enum GameAction {
    /// Go back to library
    Back,
    /// Install game with version index
    Install(usize),
    /// Uninstall game
    Uninstall,
    /// Fetch or refresh metadata
    FetchMetadata,
}

/// Game detail view
pub struct GameDetailView {
    /// Selected version index
    selected_version: usize,
    /// Game ID for metadata
    game_id: String,
    /// Refresh pending flag
    refresh_pending: bool,
    /// Error message
    error_message: Option<String>,
    /// Image texture ID if loaded
    cover_texture: Option<egui::TextureHandle>,
}

impl GameDetailView {
    /// Create a new game detail view
    pub fn new(game_id: String) -> Self {
        Self {
            selected_version: 0,
            game_id,
            refresh_pending: false,
            error_message: None,
            cover_texture: None,
        }
    }
    
    /// Update the game ID
    pub fn update_game_id(&mut self, game_id: String) {
        self.game_id = game_id;
        self.error_message = None;
        self.cover_texture = None; // Reset texture when game changes
    }

    /// Get the current game ID
    pub fn get_game_id(&self) -> &str {
        &self.game_id
    }

    /// Set refresh pending state
    pub fn set_refresh_pending(&mut self, pending: bool) {
        self.refresh_pending = pending;
    }
    
    /// Set error message
    pub fn set_error(&mut self, error: Option<String>) {
        self.error_message = error;
    }
    
    /// Show the game detail view
    pub fn show<F>(&mut self, ui: &mut egui::Ui, game: &GameInfo, is_installed: bool, metadata_handler: &MetadataHandler, mut on_action: F)
    where
        F: FnMut(GameAction),
    {
        // Navigation
        ui.horizontal(|ui| {
            if ui.button("← Back to Library").clicked() {
                on_action(GameAction::Back);
            }
            
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Refresh Metadata").clicked() {
                    on_action(GameAction::FetchMetadata);
                }
            });
        });
        
        ui.separator();
        
        // Show error if present
        if let Some(error) = &self.error_message {
            ui.label(RichText::new(format!("Error: {}", error)).color(egui::Color32::RED));
            ui.separator();
        }
        
        // Show refresh status if pending
        if self.refresh_pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Refreshing metadata...");
            });
            ui.separator();
        }
        
        // Game title
        ui.heading(&game.title);
        
        // Metadata from IGDB or game info
        let has_igdb = metadata_handler.has_igdb_metadata(&self.game_id);
        let metadata = metadata_handler.get_metadata(&self.game_id);
        
        // Basic metadata row
        ui.horizontal(|ui| {
            // Show developer/publisher based on available data
            if has_igdb {
                if let Some(metadata) = metadata {
                    if let Some(igdb_data) = &metadata.igdb_data {
                        // Get companies from IGDB
                        if let Some(companies) = &igdb_data.involved_companies {
                            let developers = companies.iter()
                                .filter(|c| c.developer)
                                .map(|c| c.company.name.as_str())
                                .collect::<Vec<_>>();
                                
                            let publishers = companies.iter()
                                .filter(|c| c.publisher)
                                .map(|c| c.company.name.as_str())
                                .collect::<Vec<_>>();
                            
                            if !developers.is_empty() {
                                ui.label(format!("Developer: {}", developers.join(", ")));
                                ui.separator();
                            }
                            
                            if !publishers.is_empty() {
                                ui.label(format!("Publisher: {}", publishers.join(", ")));
                                ui.separator();
                            }
                        }
                        
                        // Release date
                        if let Some(release_date) = igdb_data.first_release_date {
                            let date = chrono::NaiveDateTime::from_timestamp_opt(release_date as i64, 0)
                                .map(|dt| dt.format("%B %d, %Y").to_string())
                                .unwrap_or_else(|| "Unknown".to_string());
                            
                            ui.label(format!("Released: {}", date));
                        }
                    }
                }
            } else {
                // Fall back to game info
                if let Some(developer) = &game.developer {
                    ui.label(format!("Developer: {}", developer));
                    ui.separator();
                }
                
                if let Some(publisher) = &game.publisher {
                    ui.label(format!("Publisher: {}", publisher));
                    ui.separator();
                }
                
                if let Some(release_date) = &game.release_date {
                    ui.label(format!("Released: {}", release_date));
                }
            }
        });
        
        ui.separator();
        
        // Split layout for details and versions
        ui.columns(2, |columns| {
            // Left column - Game details and cover
            columns[0].vertical(|ui| {
                // Cover image from IGDB or placeholder
                if has_igdb && metadata_handler.has_cover(&self.game_id) {
                    let cover_path = metadata_handler.get_cover_path(&self.game_id);
                    self.render_cover_image(ui, &cover_path);
                } else {
                    if !has_igdb {
                        ui.vertical_centered(|ui| {
                            ui.label("No IGDB metadata available");
                            if ui.button("Fetch Metadata").clicked() {
                                on_action(GameAction::FetchMetadata);
                            }
                        });
                    } else if let Some(metadata) = metadata {
                        if let Some(igdb_data) = &metadata.igdb_data {
                            if igdb_data.cover.is_some() {
                                ui.vertical_centered(|ui| {
                                    ui.label("Cover available but not downloaded");
                                    if ui.button("Download Cover").clicked() {
                                        on_action(GameAction::FetchMetadata);
                                    }
                                });
                            } else {
                                ui.label("No cover image available");
                            }
                        }
                    } else {
                        ui.label("Cover Image (Placeholder)");
                    }
                }
                
                ui.separator();
                
                // Game description from IGDB or game info
                if has_igdb {
                    if let Some(metadata) = metadata {
                        if let Some(igdb_data) = &metadata.igdb_data {
                            // Get summary from IGDB
                            if let Some(summary) = &igdb_data.summary {
                                ui.label(RichText::new("IGDB Summary:").strong());
                                ui.separator();
                                
                                // Use scrollable area for potentially long text
                                egui::ScrollArea::vertical()
                                    .max_height(200.0)
                                    .show(ui, |ui| {
                                        ui.label(summary);
                                    });
                            }
                            
                            // Show genres if available
                            if let Some(genres) = &igdb_data.genres {
                                if !genres.is_empty() {
                                    ui.add_space(10.0);
                                    ui.label(RichText::new("Genres:").strong());
                                    let genre_list = genres.iter()
                                        .map(|g| g.name.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    
                                    ui.label(genre_list);
                                }
                            }
                            
                            // Rating if available
                            if let Some(rating) = igdb_data.total_rating {
                                ui.add_space(10.0);
                                ui.label(format!("Rating: {:.1}/100", rating));
                            }
                            
                            // Link to IGDB
                            if let Some(url) = &igdb_data.url {
                                ui.add_space(5.0);
                                ui.hyperlink_to("View on IGDB", url);
                            }
                        }
                    }
                } else {
                    // Fall back to game info
                    if let Some(description) = &game.description {
                        ui.label(RichText::new("Description:").strong());
                        ui.separator();
                        
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                ui.label(description);
                            });
                    } else {
                        ui.label("No description available.");
                    }
                }
            });
            
            // Right column - Version selection and installation
            columns[1].vertical(|ui| {
                ui.heading("Versions");
                ui.separator();
                
                // Version list
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, version) in game.versions.iter().enumerate() {
                        ui.radio_value(&mut self.selected_version, i, &version.name);
                        
                        ui.horizontal(|ui| {
                            ui.label(format!("Build: {}", version.build));
                            
                            // Show installer file count
                            let installer_count = version.files.len();
                            ui.label(format!("{} files", installer_count));
                            
                            // Show patch count
                            let patch_count = version.required_patches.len();
                            if patch_count > 0 {
                                ui.label(format!("{} patches", patch_count));
                            }
                        });
                        
                        ui.separator();
                    }
                });
                
                ui.separator();
                
                // Installation actions
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if is_installed {
                        if ui.button("Uninstall").clicked() {
                            on_action(GameAction::Uninstall);
                        }
                    } else if !game.versions.is_empty() {
                        if ui.button("Install Selected Version").clicked() {
                            on_action(GameAction::Install(self.selected_version));
                        }
                    } else {
                        ui.label("No versions available to install");
                    }
                });
            });
        });
    }
    
    /// Render cover image
    fn render_cover_image(&mut self, ui: &mut Ui, path: &PathBuf) {
        if path.exists() {
            // Try to render the actual image file
            let cover_image_rect = egui::Rect::from_min_size(
                ui.cursor().min,
                egui::vec2(200.0, 300.0)
            );

            // Try to load the image
            if self.cover_texture.is_none() {
                if let Ok(image_data) = fs::read(path) {
                    // Load the image data
                    if let Ok(image) = image::load_from_memory(&image_data) {
                        let size = [image.width() as _, image.height() as _];
                        let image_rgba = image.to_rgba8();
                        let pixels = image_rgba.as_flat_samples();
                        
                        // Create a texture
                        let texture = ui.ctx().load_texture(
                            "game_cover",
                            egui::ColorImage::from_rgba_unmultiplied(
                                size,
                                pixels.as_slice(),
                            ),
                            egui::TextureOptions::default(),
                        );
                        
                        self.cover_texture = Some(texture);
                    }
                }
            }
            
            // Display the loaded texture or a placeholder
            if let Some(texture) = &self.cover_texture {
                ui.image(texture, egui::vec2(200.0, 300.0));
            } else {
                // Fallback if loading fails
                ui.allocate_ui_at_rect(cover_image_rect, |ui| {
                    ui.painter().rect_filled(
                        cover_image_rect,
                        4.0,
                        egui::Color32::from_rgb(100, 100, 200)
                    );
                    ui.centered_and_justified(|ui| {
                        ui.label("Cover Image");
                    });
                });
            }
        } else {
            // Show placeholder if file doesn't exist
            let cover_image_rect = egui::Rect::from_min_size(
                ui.cursor().min,
                egui::vec2(200.0, 300.0)
            );
            
            ui.allocate_ui_at_rect(cover_image_rect, |ui| {
                ui.painter().rect_filled(
                    cover_image_rect,
                    4.0,
                    egui::Color32::from_rgb(100, 100, 200)
                );
                ui.centered_and_justified(|ui| {
                    ui.label("No Cover Available");
                });
            });
        }
    }
}
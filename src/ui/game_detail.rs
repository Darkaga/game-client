use eframe::egui;
use egui::{Align, Layout, RichText, Ui};
use std::path::PathBuf;
use std::fs;

use crate::repository::GameInfo;
use crate::metadata::handler::MetadataHandler;
use crate::ui::helpers; // Using our shared image-loading helper

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
    /// Cached cover texture
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
        
        // Display error if any
        if let Some(error) = &self.error_message {
            ui.label(RichText::new(format!("Error: {}", error)).color(egui::Color32::RED));
            ui.separator();
        }
        
        // Show spinner if refresh is pending
        if self.refresh_pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Refreshing metadata...");
            });
            ui.separator();
        }
        
        // Game title
        ui.heading(&game.title);
        
        // Metadata display from IGDB or fallback to game info
        let has_igdb = metadata_handler.has_igdb_metadata(&self.game_id);
        let metadata = metadata_handler.get_metadata(&self.game_id);
        
        ui.horizontal(|ui| {
            if has_igdb {
                if let Some(metadata) = metadata {
                    if let Some(igdb_data) = &metadata.igdb_data {
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
                        
                        if let Some(release_date) = igdb_data.first_release_date {
                            let date = chrono::NaiveDateTime::from_timestamp_opt(release_date as i64, 0)
                                .map(|dt| dt.format("%B %d, %Y").to_string())
                                .unwrap_or_else(|| "Unknown".to_string());
                            
                            ui.label(format!("Released: {}", date));
                        }
                    }
                }
            } else {
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
        
        // Split layout: details and version/installation
        ui.columns(2, |columns| {
            // Left column: details and cover image
            columns[0].vertical(|ui| {
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
                
                if has_igdb {
                    if let Some(metadata) = metadata {
                        if let Some(igdb_data) = &metadata.igdb_data {
                            if let Some(summary) = &igdb_data.summary {
                                ui.label(RichText::new("IGDB Summary:").strong());
                                ui.separator();
                                egui::ScrollArea::vertical()
                                    .max_height(200.0)
                                    .show(ui, |ui| {
                                        ui.label(summary);
                                    });
                            }
                            
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
                            
                            if let Some(rating) = igdb_data.total_rating {
                                ui.add_space(10.0);
                                ui.label(format!("Rating: {:.1}/100", rating));
                            }
                            
                            if let Some(url) = &igdb_data.url {
                                ui.add_space(5.0);
                                ui.hyperlink_to("View on IGDB", url);
                            }
                        }
                    }
                } else {
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
            
            // Right column: versions and installation actions
            columns[1].vertical(|ui| {
                ui.heading("Versions");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, version) in game.versions.iter().enumerate() {
                        ui.radio_value(&mut self.selected_version, i, &version.name);
                        ui.horizontal(|ui| {
                            ui.label(format!("Build: {}", version.build));
                            let installer_count = version.files.len();
                            ui.label(format!("{} files", installer_count));
                            let patch_count = version.required_patches.len();
                            if patch_count > 0 {
                                ui.label(format!("{} patches", patch_count));
                            }
                        });
                        ui.separator();
                    }
                });
                ui.separator();
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
    
    /// Render cover image using the helper function
    fn render_cover_image(&mut self, ui: &mut Ui, path: &PathBuf) {
        if self.cover_texture.is_none() {
            self.cover_texture = helpers::load_texture_from_path(ui.ctx(), path, "game_cover");
        }
        
        let cover_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(200.0, 300.0)
        );
        
        if let Some(texture) = &self.cover_texture {
            ui.image(texture, egui::vec2(200.0, 300.0));
        } else {
            ui.allocate_ui_at_rect(cover_rect, |ui| {
                ui.painter().rect_filled(
                    cover_rect,
                    4.0,
                    egui::Color32::from_rgb(100, 100, 200)
                );
                ui.centered_and_justified(|ui| {
                    ui.label("Cover Image");
                });
            });
        }
    }
}

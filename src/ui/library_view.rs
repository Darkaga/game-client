use eframe::egui;
use egui::{Align, Layout};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use image;

use crate::repository::GameInfo;
use crate::metadata::MetadataHandler;

/// View mode for the library
#[derive(PartialEq)]
pub enum ViewMode {
    /// Grid view
    Grid,
    /// List view
    List,
}

/// Library view action
pub enum LibraryAction {
    /// Select a game
    SelectGame(usize),
    /// Refresh all metadata
    RefreshAll,
}

/// Library view
pub struct LibraryView {
    /// Current view mode
    view_mode: ViewMode,
    /// Search query
    search_query: String,
    /// Cache for loaded cover textures
    cover_textures: HashMap<String, Option<egui::TextureHandle>>,
}

impl LibraryView {
    /// Create a new library view
    pub fn new() -> Self {
        Self {
            view_mode: ViewMode::Grid,
            search_query: String::new(),
            cover_textures: HashMap::new(),
        }
    }
    
    /// Show the library view
    pub fn show<F>(&mut self, ui: &mut egui::Ui, games: &[GameInfo], metadata_handler: Option<&MetadataHandler>, mut on_action: F)
    where
        F: FnMut(LibraryAction),
    {
        // Library control bar
        ui.horizontal(|ui| {
            ui.label("View:");
            
            if ui.selectable_label(self.view_mode == ViewMode::Grid, "Grid").clicked() {
                self.view_mode = ViewMode::Grid;
            }
            
            if ui.selectable_label(self.view_mode == ViewMode::List, "List").clicked() {
                self.view_mode = ViewMode::List;
            }
            
            ui.separator();
            
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search_query);
            
            // Add refresh all button
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Refresh All Metadata").clicked() {
                    on_action(LibraryAction::RefreshAll);
                }
            });
        });
        
        ui.separator();
        
        // Filter games by search query
        let filtered_games: Vec<(usize, &GameInfo)> = games
            .iter()
            .enumerate()
            .filter(|(_, game)| {
                if self.search_query.is_empty() {
                    return true;
                }
                
                let query = self.search_query.to_lowercase();
                let title = game.title.to_lowercase();
                
                title.contains(&query)
            })
            .collect();
        
        // Show games
        match self.view_mode {
            ViewMode::Grid => self.show_grid_view(ui, &filtered_games, metadata_handler, &mut on_action),
            ViewMode::List => self.show_list_view(ui, &filtered_games, metadata_handler, &mut on_action),
        }
    }
    
    /// Show grid view
    fn show_grid_view<F>(&mut self, ui: &mut egui::Ui, games: &[(usize, &GameInfo)], metadata_handler: Option<&MetadataHandler>, on_action: &mut F)
    where
        F: FnMut(LibraryAction),
    {
        const THUMBNAIL_SIZE: f32 = 160.0;
        const COVER_HEIGHT: f32 = 220.0;
        const ITEMS_PER_ROW: usize = 4;
        
        egui::ScrollArea::vertical().show(ui, |ui| {
            let available_width = ui.available_width();
            let item_width = (available_width / ITEMS_PER_ROW as f32).min(THUMBNAIL_SIZE + 20.0);
            
            // Grid layout
            let mut grid = egui::Grid::new("game_grid")
                .spacing([20.0, 20.0])
                .min_col_width(item_width)
                .max_col_width(item_width);
                
            grid.show(ui, |ui| {
                for (i, (original_index, game)) in games.iter().enumerate() {
                    // Start a new row after ITEMS_PER_ROW items
                    if i > 0 && i % ITEMS_PER_ROW == 0 {
                        ui.end_row();
                    }
                    
                    // Game card
                    ui.vertical(|ui| {
                        // Show cover image if available
                        if let Some(handler) = metadata_handler {
                            if handler.has_cover(&game.id) {
                                let cover_path = handler.get_cover_path(&game.id);
                                self.render_game_cover(ui, &game.id, &cover_path, THUMBNAIL_SIZE, COVER_HEIGHT);
                            } else {
                                // Placeholder
                                let cover_rect = egui::Rect::from_min_size(
                                    ui.cursor().min, 
                                    egui::vec2(THUMBNAIL_SIZE, COVER_HEIGHT)
                                );
                                
                                ui.allocate_ui_at_rect(cover_rect, |ui| {
                                    ui.painter().rect_filled(
                                        cover_rect,
                                        4.0,
                                        egui::Color32::from_rgb(100, 100, 200)
                                    );
                                    ui.centered_and_justified(|ui| {
                                        ui.label(&game.title);
                                    });
                                });
                            }
                        } else {
                            // Placeholder without metadata handler
                            let cover_rect = egui::Rect::from_min_size(
                                ui.cursor().min, 
                                egui::vec2(THUMBNAIL_SIZE, COVER_HEIGHT)
                            );
                            
                            ui.allocate_ui_at_rect(cover_rect, |ui| {
                                ui.painter().rect_filled(
                                    cover_rect,
                                    4.0,
                                    egui::Color32::from_rgb(100, 100, 200)
                                );
                                ui.centered_and_justified(|ui| {
                                    ui.label(&game.title);
                                });
                            });
                        }
                        
                        // Game title (truncated if too long)
                        let title = if game.title.len() > 20 {
                            format!("{}...", &game.title[..17])
                        } else {
                            game.title.clone()
                        };
                        
                        let title_response = ui.button(title);
                        
                        if title_response.clicked() {
                            on_action(LibraryAction::SelectGame(*original_index));
                        }
                        
                        // Show version count
                        ui.label(format!("{} versions", game.versions.len()));
                    });
                }
            });
        });
    }
    
    /// Show list view
    fn show_list_view<F>(&mut self, ui: &mut egui::Ui, games: &[(usize, &GameInfo)], metadata_handler: Option<&MetadataHandler>, on_action: &mut F)
    where
        F: FnMut(LibraryAction),
    {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (original_index, game) in games {
                ui.horizontal(|ui| {
                    // Show small cover thumbnail if available
                    if let Some(handler) = metadata_handler {
                        if handler.has_cover(&game.id) {
                            let cover_path = handler.get_cover_path(&game.id);
                            self.render_game_cover(ui, &game.id, &cover_path, 60.0, 80.0);
                            ui.add_space(10.0);
                        }
                    }
                    
                    // Game info
                    ui.vertical(|ui| {
                        let response = ui.selectable_label(false, &game.title);
                        
                        if response.clicked() {
                            on_action(LibraryAction::SelectGame(*original_index));
                        }
                        
                        ui.horizontal(|ui| {
                            // Show game metadata
                            if let Some(developer) = &game.developer {
                                ui.label(developer);
                                ui.separator();
                            }
                            
                            if let Some(release_date) = &game.release_date {
                                ui.label(release_date);
                                ui.separator();
                            }
                            
                            ui.label(format!("{} versions", game.versions.len()));
                        });
                    });
                });
                
                ui.separator();
            }
        });
    }
    
    /// Render game cover
    fn render_game_cover(&mut self, ui: &mut egui::Ui, game_id: &str, path: &PathBuf, width: f32, height: f32) {
        // Check if we already tried to load this texture
        if !self.cover_textures.contains_key(game_id) && path.exists() {
            // Try to load the image
            if let Ok(image_data) = fs::read(path) {
                let texture = if let Ok(image) = image::load_from_memory(&image_data) {
                    let size = [image.width() as _, image.height() as _];
                    let image_rgba = image.to_rgba8();
                    let pixels = image_rgba.as_flat_samples();
                    
                    // Create a texture
                    let texture = ui.ctx().load_texture(
                        format!("game_cover_{}", game_id),
                        egui::ColorImage::from_rgba_unmultiplied(
                            size,
                            pixels.as_slice(),
                        ),
                        egui::TextureOptions::default(),
                    );
                    
                    Some(texture)
                } else {
                    None
                };
                
                // Cache the result
                self.cover_textures.insert(game_id.to_string(), texture);
            } else {
                // Cache a None value to avoid trying to load this image again
                self.cover_textures.insert(game_id.to_string(), None);
            }
        }
        
        // Render the cover
        let cover_rect = egui::Rect::from_min_size(
            ui.cursor().min, 
            egui::vec2(width, height)
        );
        
        // Allocate the space
        ui.allocate_rect(cover_rect, egui::Sense::click());
        
        if let Some(Some(texture)) = self.cover_textures.get(game_id) {
            // Draw the texture
            ui.painter().image(
                texture.id(),
                cover_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            // Draw a placeholder
            ui.painter().rect_filled(
                cover_rect,
                4.0,
                egui::Color32::from_rgb(100, 100, 200)
            );
            // Add a small text label in the center
            ui.painter().text(
                cover_rect.center(),
                egui::Align2::CENTER_CENTER,
                "No Cover",
                egui::FontId::default(),
                egui::Color32::WHITE,
            );
        }
    }
    
    /// Clear cover texture cache
    pub fn clear_texture_cache(&mut self) {
        self.cover_textures.clear();
    }
}
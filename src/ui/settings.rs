use eframe::egui;
use log::info;
use std::path::PathBuf;

use crate::config::Config;

/// Settings view
pub struct SettingsView {
    /// Current configuration
    config: Config,
    /// Edited configuration
    edited_config: Config,
    /// Save button clicked
    save_clicked: bool,
}

impl SettingsView {
    /// Create a new settings view
    pub fn new(config: Config) -> Self {
        Self {
            config: config.clone(),
            edited_config: config,
            save_clicked: false,
        }
    }
    
    /// Show the settings view
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Config> {
        self.save_clicked = false;
        
        ui.heading("Settings");
        ui.separator();
        
        // Create tabs for different settings categories
        egui::TopBottomPanel::top("settings_tabs").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_label(true, "Repository");
                ui.selectable_label(false, "Paths");
                ui.selectable_label(false, "IGDB API");
            });
        });
        
        // Repository settings
        ui.heading("Repository Settings");
        ui.separator();
        
        ui.horizontal(|ui| {
            ui.label("Server:");
            ui.text_edit_singleline(&mut self.edited_config.repository.server);
        });
        
        ui.horizontal(|ui| {
            ui.label("Share:");
            ui.text_edit_singleline(&mut self.edited_config.repository.share);
        });
        
        ui.horizontal(|ui| {
            ui.label("Username:");
            ui.text_edit_singleline(&mut self.edited_config.repository.username);
        });
        
        ui.horizontal(|ui| {
            ui.label("Password:");
            ui.add(egui::TextEdit::singleline(&mut self.edited_config.repository.password)
                .password(true));
        });
        
        ui.horizontal(|ui| {
            ui.label("Base Directory:");
            ui.text_edit_singleline(&mut self.edited_config.repository.base_dir);
        });
        
        ui.separator();
        
        // Path settings
        ui.heading("Path Settings");
        ui.separator();
        
        // Replace the path_setting method calls with inline code to avoid multiple mutable borrows
        
        // Install Directory
        ui.horizontal(|ui| {
            ui.label("Install Directory:");
            
            let mut path_str = self.edited_config.paths.install_dir.to_string_lossy().to_string();
            if ui.text_edit_singleline(&mut path_str).changed() {
                self.edited_config.paths.install_dir = PathBuf::from(path_str);
            }
            
            if ui.button("Browse").clicked() {
                info!("Browse button clicked for Install Directory");
            }
        });
        
        // Cache Directory
        ui.horizontal(|ui| {
            ui.label("Cache Directory:");
            
            let mut path_str = self.edited_config.paths.cache_dir.to_string_lossy().to_string();
            if ui.text_edit_singleline(&mut path_str).changed() {
                self.edited_config.paths.cache_dir = PathBuf::from(path_str);
            }
            
            if ui.button("Browse").clicked() {
                info!("Browse button clicked for Cache Directory");
            }
        });
        
        // Temp Directory
        ui.horizontal(|ui| {
            ui.label("Temp Directory:");
            
            let mut path_str = self.edited_config.paths.temp_dir.to_string_lossy().to_string();
            if ui.text_edit_singleline(&mut path_str).changed() {
                self.edited_config.paths.temp_dir = PathBuf::from(path_str);
            }
            
            if ui.button("Browse").clicked() {
                info!("Browse button clicked for Temp Directory");
            }
        });
        
        ui.separator();
        
        // IGDB API settings
        ui.heading("IGDB API Settings");
        ui.separator();
        
        ui.horizontal(|ui| {
            ui.label("Client ID:");
            ui.text_edit_singleline(&mut self.edited_config.igdb.client_id);
        });
        
        ui.horizontal(|ui| {
            ui.label("Client Secret:");
            ui.add(egui::TextEdit::singleline(&mut self.edited_config.igdb.client_secret)
                .password(true));
        });
        
        ui.separator();
        
        // Save and cancel buttons
        let mut return_value = None;
        
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                self.save_clicked = true;
                info!("Saving configuration");
            }
            
            if ui.button("Cancel").clicked() {
                self.edited_config = self.config.clone();
                return_value = Some(self.config.clone());
            }
        });
        
        // If cancel was clicked, return immediately
        if return_value.is_some() {
            return return_value;
        }
        
        // Return config if save was clicked
        if self.save_clicked {
            Some(self.edited_config.clone())
        } else {
            None
        }
    }
    
    // We're not using this method anymore, but keeping it for reference
    // Instead, we've inlined the code directly in the show method
    #[allow(dead_code)]
    fn path_setting(&mut self, ui: &mut egui::Ui, label: &str, path: &mut PathBuf) {
        ui.horizontal(|ui| {
            ui.label(label);
            
            let mut path_str = path.to_string_lossy().to_string();
            if ui.text_edit_singleline(&mut path_str).changed() {
                *path = PathBuf::from(path_str);
            }
            
            if ui.button("Browse").clicked() {
                info!("Browse button clicked for {}", label);
            }
        });
    }
}
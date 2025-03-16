use eframe::egui;
use log::{info, error};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use std::collections::HashMap;

use crate::config::Config;
use crate::repository::{GameInfo, SmbConnection};
use crate::metadata::handler::{MetadataHandler, MetadataStatus};
use crate::ui::game_detail::{GameDetailView, GameAction};
use crate::ui::library_view::{LibraryView, LibraryAction};

/// Application view
pub enum AppView {
    /// Library view
    Library,
    /// Game detail view
    GameDetail(String),
    /// Settings view
    Settings,
}

/// Refresh state for tracking metadata operations
pub struct RefreshState {
    pub game_id: String,
    pub is_refreshing: bool,
    pub error: Option<String>,
}

/// Game Library App
pub struct GameLibraryApp {
    /// Current view
    view: AppView,
    /// Configuration
    config: Config,
    /// SMB connection
    smb_connection: Option<SmbConnection>,
    /// Game list
    games: Vec<GameInfo>,
    /// Library view
    library_view: LibraryView,
    /// Game detail view
    game_detail_view: Option<GameDetailView>,
    /// Selected game ID
    selected_game_id: Option<String>,
    
    // Metadata handler
    metadata_handler: Option<MetadataHandler>,
    
    // Tokio runtime for async operations
    rt: Runtime,
    
    // Metadata operation state
    refresh_states: HashMap<String, Arc<StdMutex<RefreshState>>>,
    
    // Connection state
    is_connecting: bool,
    
    // Channel for receiving games from repository (still using std channel here)
    games_receiver: Option<std::sync::mpsc::Receiver<Vec<GameInfo>>>,
    
    // Channel for metadata operations using a Tokio unbounded channel
    metadata_status_sender: Option<UnboundedSender<MetadataStatus>>,
    metadata_status_receiver: Option<UnboundedReceiver<MetadataStatus>>,
    
    // Batch operation state
    is_batch_refreshing: bool,
    batch_progress: Option<(usize, usize)>, // (completed, total)
}

impl GameLibraryApp {
    /// Create a new game library app
    pub fn new(_cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        // Create tokio runtime
        let rt = Runtime::new().expect("Failed to create runtime");
        
        let library_view = LibraryView::new();
        
        let smb_connection = Some(SmbConnection::new(config.repository.clone()));
        
        // Create channel for metadata status updates using Tokio unbounded channel
        let (metadata_tx, metadata_rx) = unbounded_channel();
        
        let mut app = Self {
            view: AppView::Library,
            config,
            smb_connection,
            games: Vec::new(),
            library_view,
            game_detail_view: None,
            selected_game_id: None,
            metadata_handler: None,
            rt,
            refresh_states: HashMap::new(),
            is_connecting: false,
            games_receiver: None,
            metadata_status_sender: Some(metadata_tx),
            metadata_status_receiver: Some(metadata_rx),
            is_batch_refreshing: false,
            batch_progress: None,
        };
        
        // Initial connection to repository
        app.connect_to_repository();
        
        app
    }
    
    /// Connect to repository
    fn connect_to_repository(&mut self) {
        if self.is_connecting {
            return;
        }
        
        self.is_connecting = true;
        
        // Create a channel to receive games (using std channel for now)
        let (tx, rx) = std::sync::mpsc::channel();
        self.games_receiver = Some(rx);
        
        // Create a new connection for the async task
        let config_clone = self.config.repository.clone();
        
        // Spawn a background task to connect and list games
        self.rt.spawn(async move {
            // Create a new connection in the async task
            let mut connection = SmbConnection::new(config_clone);
            
            // Connect to repository
            match connection.connect().await {
                Ok(_) => {
                    info!("Connected to repository");
                    
                    // List games
                    match connection.list_games().await {
                        Ok(games) => {
                            info!("Found {} games in repository", games.len());
                            
                            // Send games back to main thread
                            if let Err(e) = tx.send(games) {
                                error!("Failed to send games to main thread: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to list games: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to connect to repository: {}", e);
                }
            }
        });
    }
    
    /// Check for repository connection results
    fn check_repository_results(&mut self) {
        if let Some(receiver) = &self.games_receiver {
            // Check if we have received games from the repository
            match receiver.try_recv() {
                Ok(games) => {
                    info!("Received {} games from repository", games.len());
                    self.games = games;
                    self.is_connecting = false;
                    self.games_receiver = None; // Done receiving
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.is_connecting = false;
                    self.games_receiver = None;
                }
            }
        }
    }
    
    /// Check for metadata status updates using the Tokio unbounded channel
    fn check_metadata_status(&mut self) {
        let mut need_recreate_channel = false;
        let mut collected_statuses = Vec::new();
        
        if let Some(receiver) = &mut self.metadata_status_receiver {
            loop {
                match receiver.try_recv() {
                    Ok(status) => collected_statuses.push(status),
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        need_recreate_channel = true;
                        break;
                    }
                }
            }
        }
        
        if need_recreate_channel {
            let (tx, rx) = unbounded_channel();
            self.metadata_status_sender = Some(tx);
            self.metadata_status_receiver = Some(rx);
            
            if let Some(handler) = &mut self.metadata_handler {
                if let Some(tx) = &self.metadata_status_sender {
                    handler.set_progress_channel(tx.clone());
                }
            }
        }
        
        for status in collected_statuses {
            match status {
                MetadataStatus::Started { game_id, game_name } => {
                    info!("Started fetching metadata for {}", game_name);
                    if let Some(state) = self.refresh_states.get(&game_id) {
                        let mut state = state.lock().unwrap();
                        state.is_refreshing = true;
                        state.error = None;
                    } else {
                        let state = Arc::new(StdMutex::new(RefreshState {
                            game_id: game_id.clone(),
                            is_refreshing: true,
                            error: None,
                        }));
                        self.refresh_states.insert(game_id.clone(), state);
                    }
                    
                    if let Some(detail_view) = &mut self.game_detail_view {
                        if detail_view.get_game_id() == game_id {
                            detail_view.set_refresh_pending(true);
                        }
                    }
                }
                MetadataStatus::Success { game_id, game_name } => {
                    info!("Successfully fetched metadata for {}", game_name);
                    if let Some(state) = self.refresh_states.get(&game_id) {
                        let mut state = state.lock().unwrap();
                        state.is_refreshing = false;
                        state.error = None;
                    }
                    
                    if let Some(detail_view) = &mut self.game_detail_view {
                        if detail_view.get_game_id() == game_id {
                            detail_view.set_refresh_pending(false);
                        }
                    }
                }
                MetadataStatus::Failed { game_id, game_name, error } => {
                    error!("Failed to fetch metadata for {}: {}", game_name, error);
                    if let Some(state) = self.refresh_states.get(&game_id) {
                        let mut state = state.lock().unwrap();
                        state.is_refreshing = false;
                        state.error = Some(error.clone());
                    }
                    
                    if let Some(detail_view) = &mut self.game_detail_view {
                        if detail_view.get_game_id() == game_id {
                            detail_view.set_refresh_pending(false);
                            detail_view.set_error(Some(error));
                        }
                    }
                }
                MetadataStatus::Progress { completed, total } => {
                    self.batch_progress = Some((completed, total));
                }
                MetadataStatus::Completed { successful, failed, total } => {
                    info!("Completed metadata update: {}/{} successful, {} failed", successful, total, failed);
                    self.is_batch_refreshing = false;
                    self.batch_progress = None;
                    self.library_view.clear_texture_cache();
                }
            }
        }
    }
    
    /// Ensure metadata handler is initialized
    fn ensure_metadata_handler(&mut self) {
        if self.metadata_handler.is_none() {
            let handler = MetadataHandler::new(
                self.config.igdb.clone(),
                self.config.paths.cache_dir.clone(),
            ).expect("Failed to create metadata handler");
            
            self.metadata_handler = Some(handler);
            
            if let Some(handler) = &mut self.metadata_handler {
                if let Some(tx) = &self.metadata_status_sender {
                    handler.set_progress_channel(tx.clone());
                }
            }
            
            let handler_copy = self.metadata_handler.as_ref().unwrap().clone();
            let handler_mutex = Arc::new(Mutex::new(handler_copy));
            
            self.rt.spawn(async move {
                let mut handler = handler_mutex.lock().await;
                if let Err(e) = handler.initialize().await {
                    eprintln!("Failed to initialize metadata handler: {}", e);
                }
            });
        }
    }
    
    /// Handle game selection from library
    fn handle_game_selection(&mut self, idx: usize) {
        if let Some(game) = self.games.get(idx) {
            self.selected_game_id = Some(game.id.clone());
            self.view = AppView::GameDetail(game.id.clone());
        }
    }
    
    /// Handle library action
    fn handle_library_action(&mut self, action: LibraryAction) {
        match action {
            LibraryAction::SelectGame(idx) => self.handle_game_selection(idx),
            LibraryAction::RefreshAll => self.refresh_all_metadata(),
        }
    }
    
    /// Refresh metadata for all games
    fn refresh_all_metadata(&mut self) {
        if self.is_batch_refreshing {
            return;
        }
        
        self.ensure_metadata_handler();
        self.is_batch_refreshing = true;
        
        let game_pairs: Vec<(String, String)> = self.games
            .iter()
            .map(|game| (game.id.clone(), game.title.clone()))
            .collect();
        
        let game_pairs_clone = game_pairs.clone();
        
        if let Some(handler) = &self.metadata_handler {
            let handler_copy = handler.clone();
            let handler_mutex = Arc::new(Mutex::new(handler_copy));
            
            self.rt.spawn(async move {
                let mut handler = handler_mutex.lock().await;
                if let Err(e) = handler.update_library_metadata(&game_pairs_clone).await {
                    error!("Error in batch metadata update: {}", e);
                }
            });
        }
    }
    
    /// Handle game action
    fn handle_game_action(&mut self, action: GameAction, game_id: &str, game: &GameInfo) {
        match action {
            GameAction::Back => self.view = AppView::Library,
            GameAction::Install(version_idx) => {
                info!("Installing game: {} (version: {})", game.title, version_idx);
                // TODO: Implement installation
            }
            GameAction::Uninstall => {
                info!("Uninstalling game: {}", game.title);
                // TODO: Implement uninstallation
            }
            GameAction::FetchMetadata => {
                self.ensure_metadata_handler();
                
                let game_id = game_id.to_string();
                let game_name = game.title.clone();
                
                let state = Arc::new(StdMutex::new(RefreshState {
                    game_id: game_id.clone(),
                    is_refreshing: true,
                    error: None,
                }));
                
                self.refresh_states.insert(game_id.clone(), state.clone());
                
                if let Some(detail_view) = &mut self.game_detail_view {
                    detail_view.set_refresh_pending(true);
                    detail_view.set_error(None);
                }
                
                if let Some(handler) = &self.metadata_handler {
                    let handler_copy = handler.clone();
                    let handler_mutex = Arc::new(Mutex::new(handler_copy));
                    
                    let game_id_clone = game_id.clone();
                    let game_name_clone = game_name.clone();
                    let state_clone = state.clone();
                    
                    self.rt.spawn(async move {
                        let mut handler = handler_mutex.lock().await;
                        let result = handler.refresh_metadata(&game_id_clone, &game_name_clone).await;
                        
                        let mut state = state_clone.lock().unwrap();
                        state.is_refreshing = false;
                        
                        if let Err(e) = result {
                            state.error = Some(e.to_string());
                        }
                    });
                }
            }
        }
    }
    
    /// Render the settings view
    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        
        if ui.button("Back").clicked() {
            self.view = AppView::Library;
        }
        
        ui.separator();
        
        ui.heading("Repository Settings");
        
        let mut server = self.config.repository.server.clone();
        let mut share = self.config.repository.share.clone();
        let mut username = self.config.repository.username.clone();
        let mut password = self.config.repository.password.clone();
        let mut base_dir = self.config.repository.base_dir.clone();
        
        ui.horizontal(|ui| {
            ui.label("Server:");
            ui.text_edit_singleline(&mut server);
        });
        
        ui.horizontal(|ui| {
            ui.label("Share:");
            ui.text_edit_singleline(&mut share);
        });
        
        ui.horizontal(|ui| {
            ui.label("Username:");
            ui.text_edit_singleline(&mut username);
        });
        
        ui.horizontal(|ui| {
            ui.label("Password:");
            ui.text_edit_singleline(&mut password);
        });
        
        ui.horizontal(|ui| {
            ui.label("Base directory:");
            ui.text_edit_singleline(&mut base_dir);
        });
        
        if ui.button("Save Repository Settings").clicked() {
            self.config.repository.server = server;
            self.config.repository.share = share;
            self.config.repository.username = username;
            self.config.repository.password = password;
            self.config.repository.base_dir = base_dir;
            
            if let Err(e) = self.config.save() {
                error!("Failed to save configuration: {}", e);
            }
            
            self.smb_connection = Some(SmbConnection::new(self.config.repository.clone()));
            self.connect_to_repository();
        }
        
        ui.separator();
        
        ui.heading("Path Settings");
        
        let install_dir = self.config.paths.install_dir.clone();
        let cache_dir = self.config.paths.cache_dir.clone();
        let temp_dir = self.config.paths.temp_dir.clone();
        
        ui.horizontal(|ui| {
            ui.label("Install directory:");
            ui.text_edit_singleline(&mut install_dir.to_string_lossy().to_string());
        });
        
        ui.horizontal(|ui| {
            ui.label("Cache directory:");
            ui.text_edit_singleline(&mut cache_dir.to_string_lossy().to_string());
        });
        
        ui.horizontal(|ui| {
            ui.label("Temp directory:");
            ui.text_edit_singleline(&mut temp_dir.to_string_lossy().to_string());
        });
        
        if ui.button("Save Path Settings").clicked() {
            // TODO: Update path settings
        }
        
        ui.separator();
        
        ui.heading("IGDB API Settings");
        ui.separator();
        
        let mut client_id = self.config.igdb.client_id.clone();
        let mut client_secret = self.config.igdb.client_secret.clone();
        
        ui.horizontal(|ui| {
            ui.label("Client ID:");
            ui.text_edit_singleline(&mut client_id);
        });
        
        ui.horizontal(|ui| {
            ui.label("Client Secret:");
            ui.text_edit_singleline(&mut client_secret);
        });
        
        if ui.button("Save IGDB Settings").clicked() {
            self.config.igdb.client_id = client_id;
            self.config.igdb.client_secret = client_secret;
            
            if let Err(e) = self.config.save() {
                error!("Failed to save configuration: {}", e);
            }
            
            self.metadata_handler = None;
        }
        
        ui.separator();
        
        if ui.button("Test IGDB Connection").clicked() {
            self.ensure_metadata_handler();
            
            if let Some(handler) = &self.metadata_handler {
                let handler_copy = handler.clone();
                let handler_mutex = Arc::new(Mutex::new(handler_copy));
                
                self.rt.spawn(async move {
                    let mut handler = handler_mutex.lock().await;
                    match handler.search_game("The Witcher 3").await {
                        Ok(games) => {
                            info!("IGDB test successful: found {} games", games.len());
                            for game in games {
                                info!("  - {} (ID: {})", game.name, game.id);
                            }
                        }
                        Err(e) => {
                            error!("IGDB test failed: {}", e);
                        }
                    }
                });
            }
        }
    }
}

impl eframe::App for GameLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_repository_results();
        self.check_metadata_status();
        
        let mut game_action = None;
        let mut action_game_id = None;
        let mut action_game = None;
        let mut library_action = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.view {
                AppView::GameDetail(_) | AppView::Library => self.ensure_metadata_handler(),
                _ => {}
            }
            
            match &self.view {
                AppView::Library => {
                    ui.horizontal(|ui| {
                        ui.heading("Game Library");
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Settings").clicked() {
                                self.view = AppView::Settings;
                            }
                            
                            if ui.button("Refresh").clicked() {
                                self.connect_to_repository();
                            }
                        });
                    });
                    
                    ui.separator();
                    
                    if self.is_connecting {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Connecting to repository...");
                        });
                        ui.separator();
                    }
                    
                    if self.is_batch_refreshing {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            if let Some((completed, total)) = self.batch_progress {
                                ui.label(format!("Refreshing metadata: {}/{} games", completed, total));
                            } else {
                                ui.label("Refreshing all metadata...");
                            }
                        });
                        ui.separator();
                    }
                    
                    ui.label(format!("Found {} games", self.games.len()));
                    ui.separator();
                    
                    let lib_action = {
                        let mut action = None;
                        self.library_view.show(ui, &self.games, self.metadata_handler.as_ref(), |a| {
                            action = Some(a);
                        });
                        action
                    };
                    
                    if let Some(a) = lib_action {
                        library_action = Some(a);
                    }
                }
                AppView::GameDetail(game_id) => {
                    let game = self.games.iter().find(|g| g.id == *game_id).cloned();
                    
                    if let Some(game) = game {
                        let is_installed = false; // TODO: Check if installed
                        
                        if self.game_detail_view.is_none() {
                            self.game_detail_view = Some(GameDetailView::new(game_id.to_string()));
                        }
                        
                        if let Some(detail_view) = &mut self.game_detail_view {
                            if detail_view.get_game_id() != game_id {
                                detail_view.update_game_id(game_id.to_string());
                            }
                            
                            if let Some(state) = self.refresh_states.get(game_id) {
                                let state = state.lock().unwrap();
                                detail_view.set_refresh_pending(state.is_refreshing);
                                detail_view.set_error(state.error.clone());
                            }
                        }
                        
                        if let Some(detail_view) = &mut self.game_detail_view {
                            if let Some(metadata_handler) = &self.metadata_handler {
                                let mut action_to_take = None;
                                
                                detail_view.show(ui, &game, is_installed, metadata_handler, |action| {
                                    action_to_take = Some(action);
                                });
                                
                                if let Some(action) = action_to_take {
                                    game_action = Some(action);
                                    action_game_id = Some(game_id.clone());
                                    action_game = Some(game.clone());
                                }
                            } else {
                                ui.label("Metadata handler not initialized");
                            }
                        }
                    } else {
                        ui.heading("Game not found");
                        if ui.button("Back to Library").clicked() {
                            self.view = AppView::Library;
                        }
                    }
                }
                AppView::Settings => {
                    self.render_settings(ui);
                }
            }
        });
        
        if let Some(action) = library_action {
            self.handle_library_action(action);
        }
        
        if let (Some(action), Some(game_id), Some(game)) = (game_action, action_game_id, action_game) {
            self.handle_game_action(action, &game_id, &game);
        }
        
        ctx.request_repaint();
    }
}

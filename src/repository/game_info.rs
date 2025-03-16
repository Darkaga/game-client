use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use log::{debug, info, warn};
use regex::Regex;

/// Type of game file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum FileType {
    /// Game installer
    Installer,
    /// Game patch
    Patch,
    /// Other file
    Other,
}

/// Information about a game file
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GameFile {
    /// File name
    pub name: String,
    /// Remote path relative to repository root
    pub remote_path: String,
    /// File size in bytes
    pub size: u64,
    /// File type
    pub file_type: FileType,
}

/// Information about a game version
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GameVersion {
    /// Version name
    pub name: String,
    /// Build number
    pub build: u32,
    /// Files for this version
    pub files: Vec<GameFile>,
    /// Required patches to install from base version
    pub required_patches: Vec<GameFile>,
}

/// Information about a game
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GameInfo {
    /// Game ID (directory name)
    pub id: String,
    /// Game title
    pub title: String,
    /// Developer
    pub developer: Option<String>,
    /// Publisher
    pub publisher: Option<String>,
    /// Release date
    pub release_date: Option<String>,
    /// Game description
    pub description: Option<String>,
    /// IGDB ID
    pub igdb_id: Option<u32>,
    /// Available files
    pub files: Vec<GameFile>,
    /// Available versions
    pub versions: Vec<GameVersion>,
    /// Cover image path
    pub cover_image: Option<PathBuf>,
}

impl GameInfo {
    /// Parse metadata from !info.txt
    pub fn parse_metadata(&mut self, content: &str) {
        // Track keys we've seen to handle multi-line values
        let mut current_key: Option<String> = None;
        let mut multiline_value = String::new();
        
        for line in content.lines() {
            let line = line.trim();
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            // Check if this line starts a new key-value pair
            if let Some((key, value)) = line.split_once(':') {
                // If we were building a multiline value, save it now
                if let Some(prev_key) = current_key.take() {
                    if !multiline_value.is_empty() {
                        self.set_metadata_value(&prev_key, &multiline_value);
                        multiline_value.clear();
                    }
                }
                
                // Start a new key-value pair
                let key = key.trim().to_lowercase();
                let value = value.trim();
                
                if value.is_empty() {
                    // Start of a multiline value
                    current_key = Some(key);
                } else {
                    // Single line value
                    self.set_metadata_value(&key, value);
                }
            } else if let Some(key) = &current_key {
                // Continue multiline value
                if !multiline_value.is_empty() {
                    multiline_value.push('\n');
                }
                multiline_value.push_str(line);
            }
        }
        
        // Save any remaining multiline value
        if let Some(key) = current_key {
            if !multiline_value.is_empty() {
                self.set_metadata_value(&key, &multiline_value);
            }
        }
    }
    
    /// Set a metadata value based on key
    fn set_metadata_value(&mut self, key: &str, value: &str) {
        match key {
            "title" | "name" | "game" | "game name" => self.title = value.to_string(),
            "developer" | "dev" => self.developer = Some(value.to_string()),
            "publisher" | "pub" => self.publisher = Some(value.to_string()),
            "release" | "release date" | "date" => self.release_date = Some(value.to_string()),
            "description" | "desc" | "about" => self.description = Some(value.to_string()),
            "igdb" | "igdb_id" | "igdb id" => self.igdb_id = value.parse().ok(),
            _ => {
                // Unknown key, ignore
                debug!("Unknown metadata key: {}", key);
            }
        }
    }
    
    /// Parse available versions from files
    pub fn parse_versions(&mut self) {
        // Extract installer files
        let installer_files: Vec<&GameFile> = self.files.iter()
            .filter(|f| f.file_type == FileType::Installer)
            .collect();
            
        // Extract patch files
        let patch_files: Vec<&GameFile> = self.files.iter()
            .filter(|f| f.file_type == FileType::Patch)
            .collect();
            
        // Parse installer files to get versions
        let mut versions: Vec<GameVersion> = Vec::new();
        
        // Try several version pattern regexes to find the best match
        let version_patterns = [
            // Common GOG pattern: build_1234
            Regex::new(r"build_(\d+[a-z]?)_?\(?(\d+)?\)?").unwrap(),
            // Common version pattern: v1.2.3
            Regex::new(r"v(\d+\.\d+(\.\d+)?)").unwrap(),
            // Numeric pattern: 1.0, 2.1, etc.
            Regex::new(r"(\d+\.\d+(\.\d+)?)").unwrap(),
        ];
        
        // Map to track which files belong to which version
        let mut version_map: HashMap<String, Vec<GameFile>> = HashMap::new();
        
        // First pass: try to identify version from filenames
        for file in &installer_files {
            let file_name = file.name.to_lowercase();
            let mut found_version = false;
            
            for pattern in &version_patterns {
                if let Some(captures) = pattern.captures(&file_name) {
                    let version_str = captures.get(1).map_or("Unknown", |m| m.as_str());
                    
                    // Try to extract a build number
                    let build = if version_str.contains('.') {
                        // For version like 1.2.3, convert to a number like 10203
                        let parts: Vec<&str> = version_str.split('.').collect();
                        let mut build_num = 0;
                        
                        for (i, part) in parts.iter().enumerate() {
                            if let Ok(num) = part.parse::<u32>() {
                                build_num += num * 10u32.pow(6 - (i as u32 * 2));
                            }
                        }
                        
                        build_num
                    } else {
                        // Try to parse as a simple number
                        captures.get(2)
                            .and_then(|m| m.as_str().parse::<u32>().ok())
                            .or_else(|| version_str.parse::<u32>().ok())
                            .unwrap_or(0)
                    };
                    
                    // Create a version name
                    let version_name = if version_str.contains('.') {
                        format!("Version {}", version_str)
                    } else {
                        format!("Build {}", version_str)
                    };
                    
                    // Add to version map
                    version_map.entry(version_name)
                        .or_insert_with(Vec::new)
                        .push((**file).clone());
                    
                    found_version = true;
                    break;
                }
            }
            
            // If no version found, use a default
            if !found_version {
                version_map.entry("Default Version".to_string())
                    .or_insert_with(Vec::new)
                    .push((**file).clone());
            }
        }
        
        // Create versions from the map
        for (name, files) in version_map {
            let build = match name.as_str() {
                "Default Version" => 1,
                _ => {
                    // Try to extract a number from the version name
                    let num_regex = Regex::new(r"(\d+)").unwrap();
                    num_regex.captures(&name)
                        .and_then(|cap| cap.get(1))
                        .and_then(|m| m.as_str().parse::<u32>().ok())
                        .unwrap_or(1)
                }
            };
            
            let version = GameVersion {
                name,
                build,
                files,
                required_patches: Vec::new(),
            };
            
            versions.push(version);
        }
        
        // Second pass: assign patches to versions
        if !patch_files.is_empty() && !versions.is_empty() {
            for patch in &patch_files {
                let patch_name = patch.name.to_lowercase();
                
                // Try to match patch with version based on build numbers
                let from_to_regex = Regex::new(r"(?:patch|update).*?(?:build|v)_?(\d+[a-z]?)(?:_|\s|-).*?(?:to|-).*?(?:build|v)_?(\d+[a-z]?)").unwrap();
                
                if let Some(captures) = from_to_regex.captures(&patch_name) {
                    let from_str = captures.get(1).map_or("", |m| m.as_str());
                    let to_str = captures.get(2).map_or("", |m| m.as_str());
                    
                    let from_build = from_str.parse::<u32>().unwrap_or(0);
                    let _to_build = to_str.parse::<u32>().unwrap_or(0);
                    
                    // Find the matching version
                    for version in &mut versions {
                        if version.build == from_build {
                            version.required_patches.push((**patch).clone());
                        }
                    }
                } else {
                    // If we can't match the patch to a specific version, add it to all versions
                    for version in &mut versions {
                        version.required_patches.push((**patch).clone());
                    }
                }
            }
        }
        
        // If no versions could be identified, create a default one with all installers
        if versions.is_empty() && !installer_files.is_empty() {
            let default_files: Vec<GameFile> = installer_files.iter().map(|&f| f.clone()).collect();
            
            let version = GameVersion {
                name: "Default Version".to_string(),
                build: 1,
                files: default_files,
                required_patches: Vec::new(),
            };
            
            versions.push(version);
        }
        
        // Sort versions by build number (descending)
        versions.sort_by(|a, b| b.build.cmp(&a.build));
        
        self.versions = versions;
    }
    
    /// Get the latest version
    pub fn latest_version(&self) -> Option<&GameVersion> {
        self.versions.first()
    }
    
    /// Get a version by build number
    pub fn get_version_by_build(&self, build: u32) -> Option<&GameVersion> {
        self.versions.iter().find(|v| v.build == build)
    }
}
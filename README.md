# Game Library Manager

A desktop application for browsing, installing, and managing DRM-free games stored on a local SMB repository.

![Game Library Manager](https://raw.githubusercontent.com/your-username/game-library-manager/main/docs/images/screenshot.png)

## Features

- 🎮 Browse your game collection with grid or list view
- 🔍 Search functionality to quickly find games
- 📊 Display game metadata and artwork from IGDB
- 💿 Install and uninstall games with ease
- 🔄 Handle multiple versions of a game, including patches
- 🔌 Connect to any SMB network share
- ⚙️ Fully configurable paths and settings

## Requirements

- Windows (primary) or Linux (secondary)
- Network access to an SMB share containing your games
- IGDB API credentials (for metadata and artwork)

## Installation

### From Releases

1. Download the latest release from the [Releases page](https://github.com/your-username/game-library-manager/releases)
2. Extract the archive
3. Run the executable

### Building from Source

```bash
# Clone the repository
git clone https://github.com/your-username/game-library-manager.git
cd game-library-manager

# Build the application
cargo build --release

# Run the application
cargo run --release
```

## Setup

When you first launch the application, you'll need to configure it:

1. Go to Settings
2. Configure your SMB repository:
   - Server IP or hostname
   - Share name
   - Username and password (if required)
   - Base directory
3. Set your preferred installation directory
4. Enter your IGDB API credentials
5. Save the settings

## Repository Structure

The application expects your SMB repository to be organized in a specific way:

```
Repository/
  Windows/
    game_title_1/
      !info.txt  # Optional metadata file
      setup_game_title_1_version.exe  # Installer
      patch_game_title_1_version_to_newer_version.exe  # Patches
    game_title_2/
      ...
```

Each game has its own directory containing installers and patches. The application will automatically detect the latest version and any available patches.

## Game Installation

1. Browse the library and select a game
2. In the detail view, select the version you want to install
3. Click "Install"
4. Wait for the download and installation to complete
5. The game will be installed to your configured installation directory

## Configuration

Configuration is stored in:
- Windows: `%APPDATA%\game-library-manager\config.toml`
- Linux: `$HOME/.config/game-library-manager/config.toml`

You can edit this file manually, but it's recommended to use the Settings UI.

## Development

### Project Structure

```
game-library-manager/
├── src/
│   ├── config.rs          # Configuration handling
│   ├── repository/        # SMB connection and file operations
│   ├── metadata/          # IGDB API integration
│   ├── installer/         # Game installation logic
│   └── ui/                # User interface components
├── assets/                # Application assets
└── Cargo.toml             # Project dependencies
```

### Key Dependencies

- `eframe`/`egui` - GUI framework
- `smb2` - SMB protocol client
- `tokio` - Asynchronous runtime
- `reqwest` - HTTP client for IGDB API
- `serde` - Serialization/deserialization

## Troubleshooting

### Cannot connect to SMB repository

- Ensure the SMB server is running and accessible
- Check that the username and password are correct
- Verify that the share name is correct
- Make sure your network allows SMB connections

### Game installation fails

- Check that the installer files are valid
- Ensure you have sufficient disk space
- Verify that you have write permissions for the installation directory

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [IGDB](https://www.igdb.com/) for their comprehensive game database
- [GOG](https://www.gog.com/) for pioneering DRM-free gaming
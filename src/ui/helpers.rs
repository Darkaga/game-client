use eframe::egui;
use std::path::Path;
use std::fs;

pub fn load_texture_from_path(ctx: &egui::Context, path: &Path, texture_id: &str) -> Option<egui::TextureHandle> {
    if path.exists() {
        if let Ok(image_data) = fs::read(path) {
            if let Ok(image) = image::load_from_memory(&image_data) {
                let size = [image.width() as _, image.height() as _];
                let image_rgba = image.to_rgba8();
                let pixels = image_rgba.as_flat_samples();
                let texture = ctx.load_texture(
                    texture_id,
                    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
                    egui::TextureOptions::default(),
                );
                return Some(texture);
            }
        }
    }
    None
}

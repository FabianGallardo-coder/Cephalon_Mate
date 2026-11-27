//! Carga de assets del Warframe Fansite Kit — logos, sprites HUD, imágenes.
//! Solo se cargan los archivos necesarios (liviano).

use eframe::egui::ColorImage;
use image::{DynamicImage, ImageFormat};
use std::path::{Path, PathBuf};

/// Sheet de sprites/UI cargados del Fansite Kit.
pub struct SpriteSheet {
    /// Logo principal (para header)
    pub logo: Option<DynamicImage>,
    /// Sprites HUD por nombre (ej. "hud_corner_tl", "hud_divider", "orbis_icon")
    pub hud_sprites: std::collections::HashMap<String, DynamicImage>,
    /// Íconos de items/warframes (cache bajo demanda)
    pub icon_cache: std::sync::Mutex<std::collections::HashMap<String, DynamicImage>>,
}

impl SpriteSheet {
    /// Carga assets básicos del Fansite Kit.
    pub fn load() -> Self {
        let mut sheet = Self {
            logo: None,
            hud_sprites: std::collections::HashMap::new(),
            icon_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        };

        let kit_root = Self::kit_root();
        if !kit_root.exists() {
            eprintln!("[gui] Fansite Kit no encontrado en {:?}", kit_root);
            return sheet;
        }

        // Logo principal
        let logo_path = kit_root.join("Logos").join("Warframe_Logo.png");
        if let Ok(img) = image::open(&logo_path) {
            sheet.logo = Some(img.resize(256, 256, image::imageops::FilterType::Lanczos3));
        } else {
            // Fallback: buscar cualquier PNG en Logos/
            if let Ok(entries) = std::fs::read_dir(kit_root.join("Logos")) {
                for entry in entries.flatten() {
                    if entry.path().extension().map_or(false, |e| e == "png") {
                        if let Ok(img) = image::open(entry.path()) {
                            sheet.logo = Some(img.resize(256, 256, image::imageops::FilterType::Lanczos3));
                            break;
                        }
                    }
                }
            }
        }

        // Sprites HUD desde Images/ o Content Creation/
        let hud_dirs = [
            kit_root.join("Images").join("UI"),
            kit_root.join("Content Creation").join("UI"),
            kit_root.join("Images").join("HUD"),
        ];
        for dir in hud_dirs {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "png") {
                        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                        if let Ok(img) = image::open(&path) {
                            sheet.hud_sprites.insert(name, img);
                        }
                    }
                }
            }
        }

        eprintln!("[gui] Assets cargados: logo={}, hud_sprites={}", sheet.logo.is_some(), sheet.hud_sprites.len());
        sheet
    }

    /// Obtiene la ruta raíz del Fansite Kit.
    fn kit_root() -> PathBuf {
        // Prioridad: variable de env > path relativo > path absoluto conocido
        if let Ok(p) = std::env::var("CEPHALON_FANSITE_KIT") {
            return PathBuf::from(p);
        }
        let base = PathBuf::from("Warframe Fansite Kit");
        if base.exists() {
            return base;
        }
        // Fallback: path absoluto del usuario
        PathBuf::from(r"C:\Users\Fabian\Desktop\_Proyectos\Cephalon_Mate\Warframe Fansite Kit")
    }

    /// Obtiene un sprite HUD por nombre (con fallback).
    pub fn get_hud(&self, name: &str) -> Option<&DynamicImage> {
        self.hud_sprites.get(name)
    }

    /// Obtiene o carga un ícono de item/warframe (cache).
    pub fn get_icon(&self, name: &str) -> Option<DynamicImage> {
        // Check cache first
        if let Ok(cache) = self.icon_cache.lock() {
            if let Some(img) = cache.get(name) {
                return Some(img.clone());
            }
        }
        // Try to load from kit
        let kit_root = Self::kit_root();
        let possible_paths = [
            kit_root.join("Images").join("Items").join(format!("{}.png", name)),
            kit_root.join("Images").join("Warframes").join(format!("{}.png", name)),
            kit_root.join("Images").join("Mods").join(format!("{}.png", name)),
        ];
        for path in possible_paths {
            if let Ok(img) = image::open(&path) {
                let resized = img.resize(64, 64, image::imageops::FilterType::Lanczos3);
                if let Ok(mut cache) = self.icon_cache.lock() {
                    cache.insert(name.to_string(), resized.clone());
                }
                return Some(resized);
            }
        }
        None
    }
}

/// Convierte DynamicImage a eframe::egui::ColorImage para texture.
pub fn dynamic_to_color_image(img: &DynamicImage) -> ColorImage {
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
}
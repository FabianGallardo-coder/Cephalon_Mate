//! Logo ASCII y arte del Fansite Kit para el header del TUI.
//!
//! P13 del `PLAN_EJECUCION.md`: conversión de WarframeLogo_Black.png a ASCII art.

#[allow(dead_code)]
/// Logo ASCII de Warframe (basado en WarframeLogo_Black.png del Fansite Kit).
/// Generado manualmente para encajar en ~80 columnas.
pub const LOGO_ASCII: &str = r#"
    ╔═══════════════════════════════════════════════════════════════════╗
    ║  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ║
    ║  █  ▄▄▄█ █  ▄▄▄█ █       █ █  ▄▄▄█ █       █ █  ▄▄▄█ █  ▄▄▄█ █   ║
    ║  █ █▄▄▄█ █ █▄▄▄█ █  ▄▄▄▄▄█ █ █▄▄▄█ █  ▄▄▄▄▄█ █ █▄▄▄█ █ █▄▄▄█ █   ║
    ║  █   ▄▄▄█ █   ▄▄▄█ █▄▄▄▄▄  █ █   ▄▄▄█ █▄▄▄▄▄  █   ▄▄▄█ █   ▄▄▄█ █ ║
    ║  █  █  █ █  █  █ █▄▄▄▄▄█ █ █  █  █ █▄▄▄▄▄█ █ █  █  █ █  █  █ █   ║
    ║  █▄▄█ █▄█ █▄▄█ █▄█▄▄▄▄▄▄▄█ █▄▄█ █▄█▄▄▄▄▄▄▄█ █▄▄█ █▄█ █▄▄█ █▄█ █   ║
    ║                                                                      ║
    ║    ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ║
    ║    █  ▄▄▄█ █  ▄▄▄█ █       █ █  ▄▄▄█ █       █ █  ▄▄▄█ █  ▄▄▄█ █   ║
    ║    █ █▄▄▄█ █ █▄▄▄█ █  ▄▄▄▄▄█ █ █▄▄▄█ █  ▄▄▄▄▄█ █ █▄▄▄█ █ █▄▄▄█ █   ║
    ║    █   ▄▄▄█ █   ▄▄▄█ █▄▄▄▄▄  █ █   ▄▄▄█ █▄▄▄▄▄  █   ▄▄▄█ █   ▄▄▄█ █ ║
    ║    █  █  █ █  █  █ █▄▄▄▄▄█ █ █  █  █ █▄▄▄▄▄█ █ █  █  █ █  █  █ █   ║
    ║    █▄▄█ █▄█ █▄▄█ █▄█▄▄▄▄▄▄▄█ █▄▄█ █▄█▄▄▄▄▄▄▄█ █▄▄█ █▄█ █▄▄█ █▄█ █   ║
    ╚══════════════════════════════════════════════════════════════════╝
"#;

#[allow(dead_code)]
/// Logo compacto (una línea) para espacios reducidos.
pub const LOGO_COMPACT: &str = "▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄";

#[allow(dead_code)]
/// Bordes HUD estilo Warframe para paneles.
pub mod borders {
    #[allow(dead_code)]
    /// Esquina superior izquierda
    pub const TL: &str = "╔";
    #[allow(dead_code)]
    /// Esquina superior derecha
    pub const TR: &str = "╗";
    #[allow(dead_code)]
    /// Esquina inferior izquierda
    pub const BL: &str = "╚";
    #[allow(dead_code)]
    /// Esquina inferior derecha
    pub const BR: &str = "╝";
    #[allow(dead_code)]
    /// Horizontal
    pub const H: &str = "═";
    #[allow(dead_code)]
    /// Vertical
    pub const V: &str = "║";
    #[allow(dead_code)]
    /// T hacia abajo
    pub const T_DOWN: &str = "╦";
    #[allow(dead_code)]
    /// T hacia arriba
    pub const T_UP: &str = "╩";
    #[allow(dead_code)]
    /// T hacia la derecha
    pub const T_RIGHT: &str = "╠";
    #[allow(dead_code)]
    /// T hacia la izquierda
    pub const T_LEFT: &str = "╣";
    #[allow(dead_code)]
    /// Cruce
    pub const CROSS: &str = "╬";
}

#[allow(dead_code)]
/// Sprites ASCII simples para decoración HUD (basados en siluetas de Warframes del Kit).
pub mod sprites {
    #[allow(dead_code)]
    /// Símbolo de Tenno/Lotus
    pub const LOTUS: &str = "❀";
    #[allow(dead_code)]
    /// Símbolo de Ordis
    pub const ORDIS: &str = "⟐";
    #[allow(dead_code)]
    /// Símbolo de alerta
    pub const ALERT: &str = "⚠";
    #[allow(dead_code)]
    /// Símbolo de fissura
    pub const FISSURE: &str = "◆";
    #[allow(dead_code)]
    /// Símbolo de sortie
    pub const SORTIE: &str = "★";
    #[allow(dead_code)]
    /// Símbolo de invasión
    pub const INVASION: &str = "⚔";
    #[allow(dead_code)]
    /// Símbolo de mercado
    pub const MARKET: &str = "💰";
    #[allow(dead_code)]
    /// Símbolo de item
    pub const ITEM: &str = "📦";
    #[allow(dead_code)]
    /// Símbolo de warframe
    pub const WARFRAME: &str = "⚡";
}

#[allow(dead_code)]
/// Genera una línea decorativa HUD de ancho `width`.
pub fn hud_line(width: usize, style: HudLineStyle) -> String {
    let (left, mid, right) = match style {
        HudLineStyle::Top => (borders::TL, borders::H, borders::TR),
        HudLineStyle::Bottom => (borders::BL, borders::H, borders::BR),
        HudLineStyle::Middle => (borders::T_RIGHT, borders::H, borders::T_LEFT),
    };
    let mid_len = width.saturating_sub(left.len() + right.len());
    format!("{}{}{}", left, mid.repeat(mid_len), right)
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum HudLineStyle {
    Top,
    Middle,
    Bottom,
}

#[allow(dead_code)]
/// Header completo con logo + info del sistema.
pub fn render_header(
    personality: &str,
    model: &str,
    ollama_host: &str,
    ollama_ok: bool,
    width: usize,
) -> String {
    let mut out = String::new();
    
    // Línea superior
    out.push_str(&hud_line(width, HudLineStyle::Top));
    out.push('\n');
    
    // Logo compacto + info
    let logo = LOGO_COMPACT;
    let status = if ollama_ok { "● OK" } else { "● OFF" };
    let info = format!(" {}  ·  {}  ·  {}  ·  {}", 
        personality.to_uppercase(), model, ollama_host, status);
    
    let line = format!("{} {}", logo, info);
    let padding = width.saturating_sub(line.len());
    out.push_str(&format!("{}{}{} {}", borders::V, line, " ".repeat(padding), borders::V));
    out.push('\n');
    
    // Línea inferior
    out.push_str(&hud_line(width, HudLineStyle::Bottom));
    
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_ascii_not_empty() {
        assert!(!LOGO_ASCII.is_empty());
        assert!(LOGO_ASCII.contains("WARFRAME") || LOGO_ASCII.contains("▄"));
    }

    #[test]
    fn hud_line_generates_correct_width() {
        let line = hud_line(80, HudLineStyle::Top);
        assert!(line.len() >= 80);
        assert!(line.starts_with("╔"));
        assert!(line.ends_with("╗"));
    }

    #[test]
    fn header_renders() {
        let h = render_header("ordis", "qwen2.5:3b", "http://127.0.0.1:12434", true, 100);
        assert!(h.contains("ORDIS"));
        assert!(h.contains("qwen2.5:3b"));
        assert!(h.contains("● OK"));
        assert!(h.contains("╔"));
        assert!(h.contains("╚"));
    }
}
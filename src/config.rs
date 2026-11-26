//! Carga y defaults de `config.toml`.
//!
//! Si el archivo no existe se crea con los valores default.
//! Si existe pero está mal, se devuelve un error claro (no se rompe el arranque
//! en silencio).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuración de Cephalon Mate.
///
/// Todos los campos son opcionales al parsear (`#[serde(default)]`): un config
/// parcial hereda los defaults de los campos que falten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Personalidad activa: `ordis` | `suda` | `simaris`.
    pub personality: String,
    /// Trato al usuario: `Tenno` | `Viajero` | `Operador`.
    pub user_address: String,
    /// Modelo base de Ollama del que derivan los Modelfiles de personalidad.
    pub model: String,
    /// Host de Ollama. En este PC: `http://127.0.0.1:12434` (el 11434 está
    /// bloqueado por un excluded port range de Hyper-V).
    pub ollama_host: String,
    /// Intervalo de refresco del worldstate en background (segundos).
    pub worldstate_refresh_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            personality: "ordis".into(),
            user_address: "Tenno".into(),
            model: "qwen2.5:3b".into(),
            ollama_host: "http://127.0.0.1:12434".into(),
            worldstate_refresh_secs: 60,
        }
    }
}

impl Config {
    /// Carga el config desde `path`.
    ///
    /// - No existe → se crea con defaults y se devuelve.
    /// - Existe → se parsea (los campos faltantes heredan defaults).
    /// - Mal formado → error con contexto.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            let cfg = Self::default();
            cfg.save(path)?;
            return Ok(cfg);
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        Self::parse(&raw).map_err(|e| format!("config.toml inválido ({}): {e}", path.display()))
    }

    /// Parsea el contenido TOML de un config.
    pub fn parse(raw: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(raw)
    }

    /// Escribe el config a `path` (sobrescribe).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let raw = toml::to_string_pretty(self)
            .map_err(|e| format!("no se pudo serializar config: {e}"))?;
        std::fs::write(path, raw)
            .map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Path de archivo temporal único por test (sin dependencias externas).
    fn temp_path(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cehalon_cfg_{tag}_{nanos}.toml"))
    }

    /// Done de P02: carga defaults sin config (y lo crea).
    #[test]
    fn loads_defaults_when_missing() {
        let p = temp_path("defaults");
        assert!(!p.exists());

        let cfg = Config::load(&p).expect("debe crear config con defaults");

        assert_eq!(cfg.personality, "ordis");
        assert_eq!(cfg.user_address, "Tenno");
        assert_eq!(cfg.model, "qwen2.5:3b");
        assert_eq!(cfg.ollama_host, "http://127.0.0.1:12434");
        assert_eq!(cfg.worldstate_refresh_secs, 60);
        assert!(p.exists(), "el archivo debe haberse creado en disco");

        let _ = std::fs::remove_file(&p);
    }

    /// Done de P02: parsea un config existente con valores propios.
    #[test]
    fn parses_existing_config() {
        let p = temp_path("custom");
        std::fs::write(
            &p,
            r#"
personality = "simaris"
user_address = "Operador"
model = "qwen2.5:1.5b"
ollama_host = "http://127.0.0.1:9999"
worldstate_refresh_secs = 120
"#,
        )
        .unwrap();

        let cfg = Config::load(&p).expect("debe parsear el config existente");

        assert_eq!(cfg.personality, "simaris");
        assert_eq!(cfg.user_address, "Operador");
        assert_eq!(cfg.model, "qwen2.5:1.5b");
        assert_eq!(cfg.ollama_host, "http://127.0.0.1:9999");
        assert_eq!(cfg.worldstate_refresh_secs, 120);

        let _ = std::fs::remove_file(&p);
    }

    /// Un config parcial hereda defaults de lo que falte.
    #[test]
    fn partial_config_inherits_defaults() {
        let cfg = Config::parse(r#"personality = "suda""#).unwrap();
        assert_eq!(cfg.personality, "suda");
        assert_eq!(cfg.user_address, "Tenno"); // default heredado
        assert_eq!(cfg.worldstate_refresh_secs, 60); // default heredado
    }
}

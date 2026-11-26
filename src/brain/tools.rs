//! Tool calling: schemas, formato de llamadas y ejecutores.
//!
//! P07 del `PLAN_EJECUCION.md`: los 4 schemas y ejecutores.
//! `worldstate` usa el fetch real (P08); `buscar_item` usa SQLite real (P09);
//! `consultar_precio` usa warframe.market real (P10);
//! `buscar_guia` es stub hasta Fase 2.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::brain::items::{get_item_by_name, search_items, format_item_for_llm, get_price, format_price_for_llm, ItemRow};
use crate::brain::worldstate::{Fissure, Alert, Sortie, Invasion, WorldState};

/// Definición de tool en el formato que espera `/api/chat`.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Una llamada emitida por el modelo (formato Ollama: `function.arguments`
/// llega ya parseado como objeto, no como string).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(default)]
    pub id: String,
    pub function: ToolCallFn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFn {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Estado global cacheado del worldstate (protegido por Mutex).
static WORLDSTATE_CACHE: std::sync::Mutex<Option<WorldState>> = std::sync::Mutex::new(None);

/// Setear el cache desde el thread principal (llamado tras recibir del fetcher).
pub fn set_worldstate_cache(ws: WorldState) {
    *WORLDSTATE_CACHE.lock().unwrap() = Some(ws);
}

/// Obtener el cache (para `execute`).
fn get_worldstate_cache() -> Option<WorldState> {
    WORLDSTATE_CACHE.lock().unwrap().clone()
}

/// Conexión a la BD de items: vive en `items` (la usan el sync en background
/// y las tools).
use crate::brain::items::get_items_db;
pub fn all_tools() -> Vec<ToolDef> {
    vec![
        tool(
            "worldstate",
            "Estado en vivo de Warframe: fissuras, alertas, sortie e invasiones. Usa esto \
             siempre que el Tenno pregunte por eventos activos, fissuras, alertas, sortie o \
             invasiones.",
            json!({
                "type": "object",
                "properties": {
                    "tipo": {
                        "type": "string",
                        "enum": ["all", "fissure", "alert", "sortie", "invasion"],
                        "description": "Filtrar por tipo de evento; 'all' para todo"
                    }
                }
            }),
        ),
        tool(
            "buscar_item",
            "Busca un item de Warframe en la base de datos del proyecto (stats, crafting, \
             origen). Úsala cuando pregunten por un objeto, arma, warframe o componente.",
            json!({
                "type": "object",
                "properties": {
                    "nombre": { "type": "string", "description": "Nombre del item, p. ej. 'Soma Prime'" }
                },
                "required": ["nombre"]
            }),
        ),
        tool(
            "consultar_precio",
            "Consulta el precio actual en warframe.market de un item. Úsala cuando pregunten \
             cuánto vale algo, precio, platina o market.",
            json!({
                "type": "object",
                "properties": {
                    "item": { "type": "string", "description": "Nombre del item en warframe.market" }
                },
                "required": ["item"]
            }),
        ),
        tool(
            "buscar_guia",
            "Busca en la base de guías indexadas del proyecto (builds, farms, mecánicas). \
             Úsala cuando pregunten cómo farmear, cómo vencer un jefe o qué build usar.",
            json!({
                "type": "object",
                "properties": {
                    "pregunta": { "type": "string", "description": "La pregunta del Tenno en palabras clave" }
                },
                "required": ["pregunta"]
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, parameters: serde_json::Value) -> ToolDef {
    ToolDef {
        kind: "function".into(),
        function: ToolFunction {
            name: name.into(),
            description: description.into(),
            parameters,
        },
    }
}

/// Ejecuta una tool y devuelve el resultado como texto para el modelo.
pub fn execute(name: &str, args: &serde_json::Value) -> String {
    match name {
        "worldstate" => {
            let filter = args.get("tipo")
                .and_then(|v| v.as_str())
                .filter(|s| *s != "all");
            match get_worldstate_cache() {
                Some(ws) => format_worldstate_real(&ws, filter),
                None => "ERROR: worldstate aún no disponible (fetch en background)".into(),
            }
        }
        "buscar_item" => {
            let nombre = args.get("nombre")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if nombre.is_empty() {
                return "ERROR: parámetro 'nombre' requerido".into();
            }
            match get_items_db() {
                Some(arc) => {
                    let conn = arc.lock().unwrap();
                    // Primero intentar búsqueda exacta
                    if let Ok(Some(item)) = get_item_by_name(&conn, nombre) {
                        return format_item_for_llm(&item);
                    }
                    // Si no hay exacta, búsqueda difusa
                    match search_items(&conn, nombre, 5) {
                        Ok(items) if !items.is_empty() => {
                            let mut out = String::new();
                            for (i, item) in items.iter().enumerate() {
                                if i > 0 { out.push_str("\n---\n"); }
                                out.push_str(&format_item_for_llm(item));
                            }
                            out
                        }
                        Ok(_) => format!("No se encontraron items que coincidan con '{nombre}'."),
                        Err(e) => format!("ERROR en búsqueda: {e}"),
                    }
                }
                None => "ERROR: BD de items no inicializada".into(),
            }
        }
        "consultar_precio" => {
            let item = args.get("item")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if item.is_empty() {
                return "ERROR: parámetro 'item' requerido (slug de warframe.market, ej. 'soma_prime_set')".into();
            }
            match get_items_db() {
                Some(arc) => {
                    let conn = arc.lock().unwrap();
                    match get_price(&conn, item) {
                        Ok(price) => format_price_for_llm(&price),
                        Err(e) => format!("ERROR consultando precio de '{item}': {e}"),
                    }
                }
                None => "ERROR: BD de items no inicializada".into(),
            }
        }
        "buscar_guia" => stub("buscar_guia", "Fase 2 (RAG indexado)", args),
        other => format!(
            "ERROR: la herramienta `{other}` no existe. Usa únicamente las herramientas \
             disponibles: worldstate, buscar_item, consultar_precio, buscar_guia."
        ),
    }
}

fn format_worldstate_real(
    ws: &crate::brain::worldstate::WorldState,
    filter: Option<&str>,
) -> String {
    let mut out = String::new();
    match filter {
        Some("fissure") | None => {
            out.push_str(&ws.format_fissures(filter));
        }
        Some("alert") => {
            out.push_str(&ws.format_alerts());
        }
        Some("sortie") => {
            out.push_str(&ws.format_sortie());
        }
        Some("invasion") => {
            out.push_str(&ws.format_invasions());
        }
        _ => {
            out.push_str("WORLDSTATE (fetch real):\n");
            out.push_str(&format!("FISSURE ({}):{}", ws.fissures.len(), ws.format_fissures(None)));
            out.push_str(&format!("\nALERT ({}):{}", ws.alerts.len(), ws.format_alerts()));
            out.push_str(&format!("\nSORTIE:{}", ws.format_sortie()));
            out.push_str(&format!("\nINVASION ({}):{}", ws.invasions.len(), ws.format_invasions()));
        }
    }
    out
}

/// Etiqueta compacta para la línea verde del TUI, p. ej. `worldstate(tipo=fissure)`.
pub fn call_label(name: &str, args: &serde_json::Value) -> String {
    match args.as_object() {
        Some(obj) if !obj.is_empty() => {
            let pairs: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{k}={}", compact(v)))
                .collect();
            format!("{name}({})", pairs.join(", "))
        }
        _ => format!("{name}()"),
    }
}

fn compact(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

/// Stub honesto: la tool existe pero su implementación real llega en un paso
/// posterior. El modelo debe decirle la verdad al Tenno, no inventar datos.
fn stub(name: &str, phase: &str, args: &serde_json::Value) -> String {
    format!(
        "STUB: `{name}` todavía no tiene implementación real (llega en {phase}). \
         Argumentos recibidos: {args}. \
         Responde al Tenno que esta función aún no está disponible en esta versión."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_tools_defined() {
        let tools = all_tools();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["worldstate", "buscar_item", "consultar_precio", "buscar_guia"]
        );
        for t in &tools {
            assert_eq!(t.kind, "function");
            assert!(!t.function.description.is_empty(), "falta desc de {}", t.function.name);
            assert!(t.function.parameters.get("type").is_some(), "schema sin type");
        }
    }

    #[test]
    fn tool_call_deserializes_ollama_shape() {
        let raw = r#"{"id":"call_ytwb2dyy","function":{"index":0,"name":"worldstate","arguments":{"tipo":"fissure"}}}"#;
        let tc: ToolCall = serde_json::from_str(raw).unwrap();
        assert_eq!(tc.function.name, "worldstate");
        assert_eq!(tc.function.arguments["tipo"], "fissure");
    }

    #[test]
    fn execute_unknown_tool_errors_clearly() {
        let err = execute("hacker_tool", &json!({}));
        assert!(err.contains("no existe"), "{err}");
    }

    #[test]
    fn execute_worldstate_requires_cache() {
        // Sin cache debe dar error claro
        let out = execute("worldstate", &json!({"tipo": "fissure"}));
        assert!(out.contains("aún no disponible"), "{out}");
    }

    #[test]
    fn call_label_formats() {
        assert_eq!(call_label("worldstate", &json!({})), "worldstate()");
        assert_eq!(
            call_label("worldstate", &json!({"tipo": "fissure"})),
            "worldstate(tipo=fissure)"
        );
    }
}
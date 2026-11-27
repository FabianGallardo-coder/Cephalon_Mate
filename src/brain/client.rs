//! Cliente mínimo de Ollama: `health()` + `list_models()` + `chat()` + `chat_stream()`.
//!
//! Usa `ureq` bloqueante con timeouts diferenciados — sin async runtime (ver
//! `PLAN_EJECUCION.md`, regla 4).

use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use crate::brain::chat::{ChatMessage, StreamMsg};

pub struct OllamaClient {
    host: String,
    agent: ureq::Agent,
    #[allow(dead_code)]
    chat_agent: ureq::Agent,
    stream_agent: ureq::Agent,
}

const TIMEOUT: Duration = Duration::from_secs(5);
pub const CHAT_TIMEOUT: Duration = Duration::from_secs(120);
const STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    #[serde(default)]
    pub name: String,
}

impl OllamaClient {
    pub fn new(host: impl Into<String>) -> Self {
        let host = host.into().trim_end_matches('/').to_string();
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(TIMEOUT))
                .build(),
        );
        let chat_agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(CHAT_TIMEOUT))
                .build(),
        );
        let stream_agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_connect(Some(STREAM_CONNECT_TIMEOUT))
                .build(),
        );
        Self {
            host,
            agent,
            chat_agent,
            stream_agent,
        }
    }

    pub fn health(&self) -> Result<(), String> {
        self.agent
            .get(format!("{}/api/tags", self.host))
            .call()
            .map_err(|e| self.conn_error(e))?;
        Ok(())
    }

    pub fn list_models(&self) -> Result<Vec<String>, String> {
        let mut resp = self
            .agent
            .get(format!("{}/api/tags", self.host))
            .call()
            .map_err(|e| self.conn_error(e))?;
        let tags: TagsResponse = resp
            .body_mut()
            .read_json()
            .map_err(|e| format!("respuesta ilegible de Ollama: {e}"))?;
        let mut names: Vec<String> = tags.models.into_iter().map(|m| m.name).collect();
        names.sort();
        Ok(names)
    }

    #[allow(dead_code)]
    pub fn chat(
        &self,
        model: &str,
        messages: &[crate::brain::ChatMessage],
    ) -> Result<String, String> {
        let body = crate::brain::chat::ChatRequest {
            model,
            messages,
            stream: false,
            tools: None,
        };
        let mut resp = self
            .chat_agent
            .post(format!("{}/api/chat", self.host))
            .send_json(&body)
            .map_err(|e| self.chat_error(e))?;
        let out: crate::brain::chat::ChatResponse = resp
            .body_mut()
            .read_json()
            .map_err(|e| format!("respuesta ilegible de Ollama: {e}"))?;
        match out.message {
            Some(m) if !m.content.is_empty() => Ok(m.content),
            _ => Err("Ollama devolvió un mensaje vacío".into()),
        }
    }

    pub fn chat_stream(
        &self,
        model: &str,
        messages: &[crate::brain::ChatMessage],
        tools: Option<&[crate::brain::tools::ToolDef]>,
        mut on_token: impl FnMut(&str),
    ) -> Result<Vec<crate::brain::tools::ToolCall>, String> {
        let body = crate::brain::chat::ChatRequest {
            model,
            messages,
            stream: true,
            tools,
        };
        let resp = self
            .stream_agent
            .post(format!("{}/api/chat", self.host))
            .send_json(&body)
            .map_err(|e| self.chat_error(e))?;

        let mut reader = BufReader::new(resp.into_body().into_reader());
        let mut line = String::new();
        let mut calls: Vec<crate::brain::tools::ToolCall> = Vec::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).map_err(|e| {
                self.chat_error(ureq::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
            })?;
            if n == 0 {
                return Err("stream de Ollama terminado sin la señal `done`".into());
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Primero comprobar si es un objeto de error de Ollama
            let value: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
                return Err(err.to_string());
            }
            // Luego parsear como respuesta de chat normal
            let parsed: crate::brain::chat::ChatResponse = match serde_json::from_value(value) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if let Some(m) = parsed.message {
                if !m.tool_calls.is_empty() {
                    calls.extend(m.tool_calls);
                }
                if !m.content.is_empty() {
                    on_token(&m.content);
                }
            }
            if parsed.done {
                return Ok(calls);
            }
        }
    }

    fn conn_error(&self, e: ureq::Error) -> String {
        format!(
            "no se pudo conectar a Ollama en {} ({e})\n  → ¿Está corriendo? Revisar OLLAMA_HOST (docs/MODELOS.md)",
            self.host
        )
    }

    fn chat_error(&self, e: ureq::Error) -> String {
        match e {
            ureq::Error::Timeout(_) => format!(
                "Ollama no respondió en {}s en {} — ¿el modelo está cargando?",
                CHAT_TIMEOUT.as_secs(),
                self.host
            ),
            _ => self.conn_error(e),
        }
    }

    /// Crea un modelo en Ollama desde un Modelfile (POST /api/create).
    /// Usado por `/personality` y la GUI para crear `cehalon-<name>` on-demand.
    pub fn create_model(&self, name: &str, modelfile: &str) -> Result<(), String> {
        let body = serde_json::json!({
            "name": name,
            "modelfile": modelfile
        });
        let resp = self
            .chat_agent
            .post(format!("{}/api/create", self.host))
            .send_json(&body)
            .map_err(|e| self.conn_error(e))?;

        // /api/create devuelve streaming NDJSON; consumirlo hasta done
        let mut reader = BufReader::new(resp.into_body().into_reader());
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).map_err(|e| {
                format!("leyendo respuesta create: {e}")
            })?;
            if n == 0 {
                break; // EOF
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let value: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
                return Err(format!("Ollama create error: {err}"));
            }
            if value.get("status").and_then(|s| s.as_str()) == Some("success") {
                return Ok(());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_normalizes_trailing_slash() {
        let c = OllamaClient::new("http://127.0.0.1:12434/");
        assert_eq!(c.host(), "http://127.0.0.1:12434");
    }

    #[test]
    fn health_fails_cleanly_when_unreachable() {
        let c = OllamaClient::new("http://127.0.0.1:1");
        let err = c.health().unwrap_err();
        assert!(err.contains("OLLAMA_HOST"), "el error debe sugerir revisar OLLAMA_HOST: {err}");
    }

    /// E2E real: chat con Ollama corriendo (correr a mano, requiere el server).
    ///   cargo test -- --ignored chat_e2e
    #[test]
    #[ignore = "requiere Ollama corriendo en 12434"]
    fn chat_e2e_hola() {
        let c = OllamaClient::new("http://127.0.0.1:12434");
        let reply = c
            .chat(
                "cehalon-ordis",
                &[crate::brain::ChatMessage::user("hola, en una frase")],
            )
            .expect("Ollama debe responder");
        assert!(!reply.trim().is_empty(), "la respuesta no puede ser vacía");
        println!("ordis respondió: {reply}");
    }

    /// E2E del stream: debe llegar en varios fragmentos (no en uno solo).
    ///   cargo test -- --ignored chat_stream_e2e
    #[test]
    #[ignore = "requiere Ollama corriendo en 12434"]
    fn chat_stream_e2e_hola() {
        let c = OllamaClient::new("http://127.0.0.1:12434");
        let mut chunks = Vec::new();
        let calls = c
            .chat_stream(
                "cehalon-ordis",
                &[crate::brain::ChatMessage::user("hola, en una frase")],
                None,
                |t| chunks.push(t.to_string()),
            )
            .expect("el stream debe terminar bien");
        assert!(calls.is_empty(), "sin tools no debe pedir ninguna");
        let full: String = chunks.concat();
        assert!(!full.trim().is_empty(), "respuesta vacía");
        assert!(chunks.len() > 1, "esperaba >1 fragmento, llegaron {}", chunks.len());
        println!("stream: {} fragmentos, {} chars", chunks.len(), full.chars().count());
    }

    /// E2E de P07: con tools disponibles, "¿qué fissuras hay?" debe producir
    /// un `worldstate()` y el segundo pase debe responder con esos datos.
    ///   cargo test -- --ignored tool_loop_e2e
    #[test]
    #[ignore = "requiere Ollama corriendo en 12434"]
    fn tool_loop_e2e_worldstate() {
        let c = OllamaClient::new("http://127.0.0.1:12434");
        let all = crate::brain::tools::all_tools();
        let mut messages = vec![crate::brain::ChatMessage::user(
            "¿qué fissuras hay ahora? (por favor, usa la herramienta worldstate)",
        )];

        // Pase 1: el modelo debe pedir la tool.
        // Usamos qwen2.5:3b (base) porque cehalon-ordis responde en personaje sin llamar tools.
        let calls = c
            .chat_stream("qwen2.5:3b", &messages, Some(&all), |_| {})
            .expect("pase 1");
        assert!(!calls.is_empty(), "el modelo no pidió ninguna tool");
        assert_eq!(calls[0].function.name, "worldstate", "esperaba worldstate()");
        println!("pase 1 pidió: {}", crate::brain::tools::call_label(&calls[0].function.name, &calls[0].function.arguments));

        // Ejecutar y devolver.
        messages.push(crate::brain::ChatMessage::assistant_tools(calls.clone()));
        for call in &calls {
            let out = crate::brain::tools::execute(&call.function.name, &call.function.arguments);
            messages.push(crate::brain::ChatMessage::tool_result(&call.function.name, out));
        }

        // Pase 2: respuesta final SIN pedir más tools.
        let mut final_text = String::new();
        let calls2 = c
            .chat_stream("qwen2.5:3b", &messages, Some(&all), |t| final_text.push_str(t))
            .expect("pase 2");
        assert!(calls2.is_empty(), "no debe pedir tools otra vez");
        assert!(!final_text.trim().is_empty(), "respuesta final vacía");
        assert!(
            final_text.to_lowercase().contains("fissura") || final_text.to_lowercase().contains("lith"),
            "la respuesta debe usar los datos del mock: {final_text}"
        );
        println!("pase 2 respondió: {final_text}");
    }
}

impl OllamaClient {
    /// Host efectivo (útil para el header del TUI y mensajes de error).
    pub fn host(&self) -> &str {
        &self.host
    }
}
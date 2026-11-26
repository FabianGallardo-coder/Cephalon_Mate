//! Chat simple con Ollama (`/api/chat`, sin streaming ni tools).
//!
//! P05 del `PLAN_EJECUCION.md`: bloquea mientras el modelo responde.

use serde::{Deserialize, Serialize};

use super::tools::ToolCall;

/// Mensaje de una conversación, en el formato de la API de Ollama.
///
/// Incluye los campos opcionales del protocolo de tools: `tool_calls` en
/// mensajes del asistente y `tool_name` en respuestas de tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }

    /// Mensaje del asistente que pide ejecutar tools (contenido vacío).
    pub fn assistant_tools(calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(calls),
            tool_name: None,
        }
    }

    /// Resultado devuelto a una tool (`role: "tool"`).
    pub fn tool_result(tool_name: &str, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            tool_calls: None,
            tool_name: Some(tool_name.into()),
        }
    }
}

/// Cuerpo del request a `/api/chat`.
#[derive(Serialize)]
pub(super) struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: &'a [ChatMessage],
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<&'a [super::tools::ToolDef]>,
}

/// Respuesta de `/api/chat` (solo leemos `message.content`).
#[derive(Debug, Deserialize)]
pub(super) struct ChatResponse {
    #[serde(default)]
    pub message: Option<ChatMsg>,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ChatMsg {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<super::tools::ToolCall>,
}

/// Mensaje emitido por el stream de `/api/chat` (NDJSON, uno por línea).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamMsg {
    /// Fragmento de texto del modelo.
    Token(String),
    /// El modelo pidió ejecutar una tool (etiqueta para la línea verde).
    Tool(String),
    /// El modelo terminó la respuesta.
    Done,
    /// Error de red o de parseo durante el stream.
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_in_ollama_format() {
        let m = ChatMessage::user("hola");
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "hola");
    }

    #[test]
    fn assistant_roundtrip() {
        let m = ChatMessage::assistant("¡Hola Tenno!");
        let back: ChatMessage = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back.role, "assistant");
        assert_eq!(back.content, "¡Hola Tenno!");
    }
}

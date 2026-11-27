# Arquitectura — Cephalon Mate

Diseño técnico por capas. Fase 1 = TUI; Fase 3 = mascota (misma base, otra cara).

## Diagrama

```
┌─────────────────────────────────────────────────────────────┐
│  CAPA INTERFAZ                                              │
│  Fase 1: TUI (ratatui + crossterm)                          │
│    · historial de chat con streaming                         │
│    · panel de estado (modelo, personalidad, worldstate)      │
│    · input con historial de comandos                         │
│  Fase 3: mascota (egui o Tauri, ventana flotante)            │
│    · sprite animado, always-on-top, transparente             │
│    · sonidos del Fansite Kit                                 │
├─────────────────────────────────────────────────────────────┤
│  CAPA CEREBRO — Ollama (localhost:12434)                     │
│    · Modelfile según personalidad (ver docs/PERSONALIDADES)  │
│    · system prompt + parámetros (temperature, num_ctx)       │
│    · tool calling: el modelo elige qué tool usar             │
│    · streaming de tokens hacia la interfaz                   │
├─────────────────────────────────────────────────────────────┤
│  CAPA TOOLS (funciones Rust expuestas al modelo)             │
│    · buscar_item(nombre)        → SQLite items WFCD          │
│    · donde_encontrar(nombre)    → drops/polaridades/recetas   │
│    · consultar_precio(item)     → warframe.market (+caché)   │
│    · worldstate()               → alertas/fissuras/sortie    │
│    · buscar_guia(query)         → RAG (embeddings, stub)     │
├─────────────────────────────────────────────────────────────┤
│  CAPA DATOS — SQLite (data/cehalon_items.db)                 │
│    · items, crafting, drops, polaridades  (sync WFCD)        │
│    · precios warframe.market             (TTL ~5 min)       │
│    · worldstate en memoria               (refresh ~60 s,     │
│      no se persiste)                                         │
│    · embeddings (nomic-embed-text)   (índice RAG, Fase 2)    │
└─────────────────────────────────────────────────────────────┘
```

## Decisiones técnicas

| Área | Elección | Por qué |
|------|----------|---------|
| TUI | `ratatui` + `crossterm` | Estándar de facto en Rust TUI, cross-platform |
| HTTP | `ureq` (blocking, con `json`) | Ultraliviano, **sin tokio ni async runtime** — ver `PLAN_EJECUCION.md` |
| Concurrency | `std::thread` + `mpsc` | Suficiente para worldstate en background y streaming |
| DB | `rusqlite` (bundled) | Cero dependencias externas, un solo binario |
| Ollama client | HTTP directo a `/api/chat` con `tools` | Evita crates abandonados; API simple y estable |
| Embeddings | `nomic-embed-text` vía Ollama `/api/embeddings` | Ya instalado, buen soporte multilingüe |
| Serialización | `serde` + `serde_json` | Estándar |
| Config | `config.toml` (crate `toml`) | Simple, humano-editable |

## Flujo de una consulta

1. Usuario escribe en el TUI.
2. **Prefetch por intención** (`src/brain/intent.rs`): si la pregunta pide datos
   de un item, drop, stats o polaridades, se ejecuta la tool correspondiente
   *antes* de consultar al modelo y su salida se inyecta como bloque de datos
   verificados.
3. Se arma `/api/chat` con: system prompt de la personalidad activa, historial,
   lista de tools y (si hubo prefetch) los datos verificados.
4. Ollama responde (posiblemente con una llamada a tool).
5. Si hay tool call → se ejecuta (SQLite/API) → resultado → segundo pase al modelo.
6. Respuesta final en streaming al TUI.

## Comandos TUI (Fase 1)

| Comando | Acción |
|---------|--------|
| `/personality ordis\|suda\|simaris` | Cambia personalidad en vivo (recarga Modelfile) |
| `/model <nombre>` | Cambia modelo Ollama |
| `/status` | Estado: modelo, personalidad, worldstate y tools disponibles |
| `/worldstate` | Fuerza una actualización del worldstate (en background) |
| `/sync` | Re-sincroniza la BD de items con WFCD |
| `/clear` | Limpia historial |
| `/help` | Lista de comandos |
| `/quit` | Sale |

## Fase 3 (mascota) — notas

- Candidatos: `egui` (puro Rust, transparente + always-on-top vía `winit`) o Tauri (web stack). Recomendación inicial: **egui** para mantener un solo binario.
- La mascota consume la misma capa de cerebro/tools; no duplica lógica.
- Audio: `rodio` para reproducir clips del Fansite Kit.

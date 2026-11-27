# PROYECTO — Cephalon Mate

> Documento maestro del proyecto. Todo agente o modelo que trabaje acá debe leer este archivo primero.

## Idea principal

**Cephalon Mate** es un asistente personal de Warframe con IA **100% local** (Ollama), escrito en **Rust**, que se presenta como un Cephalon (la inteligencia artificial del juego) y responde consultas sobre:

1. **Info en vivo** — fissuras, alertas, invasiones, sorties, precios de warframe.market.
2. **Conocimiento del juego** — items, crafting, drops, builds, guías ("¿cómo farmear X?", "¿qué necesito para forjar Y?").

Primera entrega: un **TUI chat funcional**. Segunda entrega: una **mascota de escritorio** (ventana flotante con sprite y voz del Fansite Kit).

## Decisiones tomadas (historial)

| # | Decisión | Estado |
|---|----------|--------|
| 1 | Repo en `Desktop\_Proyectos\Cephalon_Mate` | ✅ |
| 2 | Lenguaje: Rust (1.98.1 instalado) | ✅ |
| 3 | Fase 1 = TUI chat funcional; la mascota viene después | ✅ |
| 4 | Alcance doble: info en vivo + conocimiento indexado (RAG) | ✅ |
| 5 | IA local vía Ollama, sin nube | ✅ |
| 6 | Personalidad configurable (Ordis / Suda / Simaris) con Modelfile propio según selección | ✅ |
| 7 | El usuario se llamará **Tenno**, **Viajero** u **Operador** (configurable) | ✅ |
| 8 | Modelo principal: universal y de bajos recursos (ver `docs/MODELOS.md`) | ✅ |
| 9 | Assets visuales/sonoros del **Warframe Fansite Kit oficial** (Digital Extremes) | ✅ |
| 10 | **No se escribe código sin permiso explícito del usuario** | ✅ vigente |

## Fases

### Fase 0 — Documentación y assets ✅
- Esta documentación. ✅
- ~~Descargar del Fansite Kit~~ → **hecho**: `C:\Users\Fabian\Desktop\_Proyectos\Cephalon_Mate\Warframe Fansite Kit` (2.320 archivos, kit completo).
- Escribir los 3 Modelfiles de personalidad. ✅ (`modelfiles/`)
- Elegir personalidad por defecto. ✅ (`ordis`, en `config.toml`)

### Fase 1 — TUI chat funcional ✅ *(v1.0.0 publicada el 2026-11-26)*
- TUI con `ratatui` + `crossterm`: historial de chat, indicador de streaming, panel de estado.
- Cliente Ollama con **tool calling**: el modelo decide cuándo consultar datos reales.
- Tools: `buscar_item`, `consultar_precio`, `worldstate`, `donde_encontrar`, `buscar_guia` (RAG, stub).
- SQLite como caché local (items WFCD + worldstate + embeddings).
- Refresco periódico del worldstate (~60 s).
- Suite de QA de datos (`--qa`) y QA end-to-end con Ollama (`scripts/qa_ollama.ps1`).

### Fase 2 — Conocimiento del juego (RAG)
- Indexar fuentes de conocimiento (wikis WFCD, datos extraídos) con `nomic-embed-text`.
- Retrieval en SQLite para preguntas abiertas.
- Citas/fuente en las respuestas para evitar alucinaciones.

### Fase 3 — La mascota
- Ventana flotante transparente siempre encima (candidatos: `egui` o Tauri) con sprite del Fansite Kit.
- Sonidos de voz del kit al responder (Ordis con sus frases).
- El TUI sigue siendo el motor; la mascota es la cara.

## Arquitectura resumida

```
┌──────────────────────────────────────────────────────┐
│  Interfaz (Fase 1: TUI ratatui — Fase 3: mascota)    │
├──────────────────────────────────────────────────────┤
│  Cerebro: Ollama + Modelfile de personalidad          │
│  (system prompt por personalidad + tool calling)      │
├──────────────────────────────────────────────────────┤
│  Tools: buscar_item · donde_encontrar · consultar_precio   │
│         worldstate · buscar_guia (RAG)                     │
├──────────────────────────────────────────────────────┤
│  Datos: SQLite  ←  WFCD API · Worldstate · wfmkt      │
│         embeddings (nomic-embed-text)                 │
└──────────────────────────────────────────────────────┘
```

Detalle en `docs/ARQUITECTURA.md`.

## Reglas para agentes de IA

1. **Leer este archivo y `AGENTS.md` antes de tocar nada.**
2. Prohibido escribir código sin permiso explícito del usuario (regla 10 del historial).
3. Los datos de juego siempre vienen de las APIs/fuentes listadas en `docs/DATOS_Y_REPOS.md` — nunca inventar números (precios, stats, drops).
4. Respetar la personalidad activa definida en `config.toml` y `docs/PERSONALIDADES.md`.
5. Mantener el proyecto compatible con equipos de bajos recursos (ver `docs/MODELOS.md`).

## Enlaces clave

- Fansite Kit oficial: https://digitalextremesltd.app.box.com/s/vh2u41yhdlgp3girffucbynadoi33173
- WFCD (Warframe Community Developers): https://github.com/WFCD
- warframe.market API: https://api.warframe.market
- Ollama: https://ollama.com

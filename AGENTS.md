# AGENTS.md — Cephalon Mate

Repositorio de un asistente de Warframe con IA local en Rust. **Leer primero `PROYECTO.md`** (documento maestro con decisiones e historial).

## Reglas de oro

1. **NO escribir código sin permiso explícito del usuario.** El proyecto está en fase de diseño/ideación. El usuario aprueba cada fase antes de que haya una línea de código.
2. **Nunca inventar datos de juego.** Precios, stats, drops, tiempos de craft y horarios de worldstate salen de las APIs/fuentes de `docs/DATOS_Y_REPOS.md`. Si no hay dato, decir "no tengo ese dato" o consultarlo vía tool.
3. **Respetar la personalidad activa** (`config.toml` → `personality`) y el trato al usuario (`Tenno` / `Viajero` / `Operador`), definidos en `docs/PERSONALIDADES.md`.
4. **Priorizar bajos recursos.** El default es un modelo liviano (`qwen2.5:3b`, ver `docs/MODELOS.md`). No introducir dependencias pesadas sin justificación.

## Contexto del entorno

- Windows, Rust 1.98.1 + cargo instalados.
- Ollama 0.40.x con modelos locales (qwen2.5, llama3.1:8b, nomic-embed-text para embeddings).
- Proyecto hermano relacionado: `Desktop\_Proyectos\repositorios_dudas` (RepoCurator AI, Python) — no confundir repos.

## Comandos (cuando existan)

```powershell
cargo build            # compilar
cargo run --release    # arrancar TUI
cargo test             # tests
```

## Estructura

Ver `docs/ESTRUCTURA.md`. Resumen: `src/` (código futuro), `modelfiles/` (Modelfiles de personalidad), `assets/` (Fansite Kit), `data/` (SQLite, ignorado por git), `docs/` (esta documentación).

## Fuentes de datos

Solo las listadas en `docs/DATOS_Y_REPOS.md`: WFCD (`warframe-items`, `warframe-worldstate-data`, `warframe-worldstate-parser`), API de worldstate, warframe.market. Licencia de assets: Warframe Fansite Kit de Digital Extremes (uso para fansites/comunidad).

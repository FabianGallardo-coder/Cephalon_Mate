# PLAN DE EJECUCIÓN — Fase 1: TUI Chat funcional

> **Reglas del juego**
> 1. Un paso por vez. Al terminar: verificar ✓, tachar, reportar.
> 2. **Nunca repetir** un paso tachado — si algo se rompe después, se abre un paso nuevo aparte (`FIX-*`).
> 3. Avanzar solo con permiso explícito del usuario.
> 4. Stack liviano obligatorio: **sin tokio, sin async runtime** — `std::thread` + `mpsc` + event loop con `poll`.
> 5. Cada paso define su **Criterio de done** (verificable) antes de darse por terminado.

## Stack decidido (liviano y responsive)

| Crate | Para qué | Por qué este |
|-------|----------|--------------|
| `ratatui` + `crossterm` | TUI | Estándar, sin dependencias pesadas |
| `ureq` (feature `json`) | HTTP a Ollama/APIs | Blocking, ~0 deps, sin runtime |
| `serde` + `serde_json` | Serialización | Obligatorio |
| `toml` | config.toml | Mínimo |
| `rusqlite` (feature `bundled`) | SQLite | Un binario, cero instalación |

Threads: `std::thread` + `std::sync::mpsc` para worldstate en background y streaming.
Render: event loop con `crossterm::event::poll(timeout)` → redraw bajo demanda (sin burn de CPU).

---

## Pasos

### Bloque A — Andén (arranque)

- [x] **P01 — `cargo init` + dependencias** ✅ *2026-11-25*
  - Crear `Cargo.toml` con las 5 dependencias de la tabla, `.gitignore` (`/target`, `/data`, `/assets`, `*.db`).
  - *Done:* `cargo build` compila sin errores, binario arranca y sale con `Ctrl+C`.
  - **Resultado:** paquete `cehalon-mate` (edition 2024). Deps: `ratatui 0.30.2`, `crossterm 0.29`, `ureq 3.5.2 (json)`, `serde 1.0.229`, `serde_json`, `toml 1.2.3`, `rusqlite 0.43 (bundled)`. Build 34 s, `cargo run` → `Hello, world!` exit=0.
  - **Nota:** el primer intento se ejecutó en el cwd de sesión equivocado (`repositorios_dudas`) — limpiado por completo (Cargo.toml/lock/src/.gitignore revertidos, archivos Python intactos). Siempre usar `workdir=Cephalon_Mate`.

- [x] **P02 — `config.toml` + loader con defaults** ✅ *2026-11-25*
  - Esquema: `personality`, `user_address`, `model`, `ollama_host` (default `http://127.0.0.1:12434`), `worldstate_refresh_secs`.
  - `src/config.rs`: carga el archivo si existe; si no, usa defaults y lo crea.
  - *Done:* test unitario que carga defaults sin config y parsea un config existente.
  - **Resultado:** `Config` con `#[serde(default)]` (config parcial hereda defaults), `load`/`parse`/`save`, config.toml shipped con comentarios. Tests: `loads_defaults_when_missing` ✓ · `parses_existing_config` ✓ · `partial_config_inherits_defaults` ✓ (3/3). `cargo run` muestra el config cargado, exit=0.

- [x] **P03 — Cliente Ollama mínimo** ✅ *2026-11-25*
  - `src/brain/client.rs`: `health()` (`/api/tags`) y `list_models()`.
  - Manejo de error claro si Ollama no responde (mensaje: revisar OLLAMA_HOST).
  - *Done:* test de integración manual: binario imprime los modelos instalados (`cehalon-*`, `qwen2.5:*`).
  - **Resultado:** `OllamaClient` con `ureq` (timeout 5s, host normalizado sin `/` final). `health()` + `list_models()` + error con sugerencia `OLLAMA_HOST`. Tests: `new_normalizes_trailing_slash` ✓ · `health_fails_cleanly_when_unreachable` ✓ (5/5 en total). `cargo run` → Ollama OK en 12434 y lista los 5 modelos (3× `cehalon-*` + qwen2.5), exit=0.

### Bloque B — Primer chat end-to-end

- [x] **P04 — Esqueleto TUI (render estático)** ✅ *2026-11-26*
  - Layout: header (identidad+estado) / panel izq. placeholder / chat / input / footer hints.
  - Tema: fondo negro, cian Warframe, bordes HUD. Resize responsive.
  - *Done:* `cargo run` muestra el layout completo y reacciona a resize sin romperse.
  - **Resultado:** `src/ui.rs` con `UiState` + paleta RGB Warframe (BG/ACCENT/ALERT/SORTIE/OK) y los 5 bloques (header, WORLDSTATE, CHAT, input con placeholder, footer hints). `src/main.rs` con event loop (`poll` 200 ms, redraw bajo demanda), raw mode + alt screen con restauración garantizada, Ctrl+C para salir. Tests: `renders_full_layout` ✓ · `survives_resize` ✓ (20×6, 60×15, 160×50) — **7/7 total**. Verificación manual: binario vivo 3 s en consola real renderizando los 5 bloques, header muestra `● OK` + `ORDIS` con Ollama arriba.

- [x] **P05 — Input + chat sin tools** ✅ *2026-11-26*
  - Teclado (Enter envía, Backspace, flechas historial), burbuja usuario + respuesta del modelo en el panel chat.
  - Sin streaming todavía: bloquea con spinner mientras responde.
  - *Done:* escribir "hola" → respuesta del Cephalon en el chat. **Primer chat funcional.**
  - **Resultado:** `brain/chat.rs` (`ChatMessage` + request/response Ollama) y `OllamaClient::chat()` (agent aparte con timeout 120 s, `send_json`). `UiState` con `messages`/`thinking`/`history`/`history_idx`; chat dinámico con etiqueta `◆ ORDIS`, wrap de palabras, auto-scroll y estado vacío. Bucle de eventos con chars/Backspace/Enter/flechas ↑↓ (historial) + Ctrl+C. Selección automática de `cehalon-<personalidad>` con fallback al modelo base.
  - **Verificación:** 9/9 tests + `chat_e2e_hola` **ignorado→pasado** (Ordis respondió en 16.7 s con `cehalon-ordis`). TUI arranca vivo sin stderr y muestra el chat en estado vacío.
  - ⚠️ **Prueba interactiva en tus manos:** `cargo run` → escribe `hola` → Enter. Si algo falla, se abre `FIX-01` (no se reabre P05).

- [x] **P06 — Streaming de tokens** ✅ *2026-11-26*
  - NDJSON de `/api/chat` leído línea a línea en thread + canal al event loop → texto aparece token a token.
  - *Done:* la respuesta se escribe en vivo (sin congelar el input).
  - **Resultado:** `StreamMsg` (Token/Done/Error) + `OllamaClient::chat_stream()` (agent con solo `timeout_connect` 5 s, sin global para no cortar respuestas largas, `BufReader::read_line` ignorando líneas rotas). `send_chat` lanza un `std::thread` con cliente propio y devuelve el `Receiver`; el event loop hace `try_recv()` en cada tick → **nunca bloquea**. `UiState.pending` pinta el texto creciendo con cursor `▌`; `Done` consolidado en el chat; input **sigue vivo** durante la respuesta (solo Enter se bloquea para no enviar 2 mensajes a la vez).
  - **Verificación:** 9/9 tests + `chat_stream_e2e_hola` **pasado** (>1 fragmento, 32.7 s con Ordis). TUI arranca vivo, sin stderr.

### Bloque C — Datos reales

- [x] **P07 — Tool calling: loop completo** ✅ *2026-11-26*
  - Definir schemas de las 4 tools (`buscar_item`, `consultar_precio`, `worldstate`, `buscar_guia` — esta última stub).
  - Loop: modelo pide tool → se ejecuta → resultado → segundo pase → respuesta final. Línea de tools mostrada en verde.
  - *Done:* "¿qué fissuras hay?" dispara `worldstate()` (mock OK en este paso) y el modelo responde con esos datos.
  - **Resultado:** `src/brain/tools.rs` con 4 schemas (`worldstate`, `buscar_item`, `consultar_precio`, `buscar_guia`), `execute()` con mock honesto para `worldstate` y stubs para el resto, `call_label()` para la línea verde (`worldstate(tipo=fissure)`). `ChatMessage` extendido con `tool_calls` y `tool_name`. `chat_stream()` devuelve `Vec<ToolCall>` para el loop. `send_chat` en `main.rs` hace el loop completo (máx. 4 rondas anti-bucle), ejecuta tools y devuelve resultados al modelo. Línea verde `⚙ tools: worldstate(tipo=fissure)` en el TUI.
  - **Verificación:** 14/14 tests unitarios + `tool_loop_e2e_worldstate` **pasado** (33 s, modelo base `qwen2.5:3b` llama `worldstate()`, segundo pase responde con datos del mock). TUI arranca vivo sin stderr.

- [x] **P08 — Worldstate en vivo** ✅ *2026-11-26*
  - Fetch real (`api.warframestat.us`) en thread con `mpsc`, refresh configurable (`worldstate_refresh_secs` en config.toml).
  - Panel izquierdo: fissuras (tier/nodo/misión/tags STORM/STEEL PATH/tiempo), alerts (misión/nodo/facción/recompensa/tiempo), sortie (3 misiones + modificadores + boss/facción/tiempo), invasions (descripción/facciones/recompensas/% completado). Header muestra "actualizado hace Xs".
  - *Done:* panel muestra datos reales y se actualiza sin parpadear el resto del UI.
  - **Resultado:** `src/brain/worldstate.rs` fetcher thread con `mpsc`, parseo tipado (`FissureRaw`, `AlertRaw`, `SortieRaw`, `InvasionRaw`), formateo `format_fissures/alerts/sortie/invasions` + `format_remaining` (chrono UTC). `main.rs` spawn al arrancar + poll en event loop → `UiState.worldstate`. `ui.rs` render dinámico con colores Warframe.

- [x] **P09 — SQLite + sync WFCD + tool `buscar_item`** ✅ *2026-11-26*
  - Esquema `items` en SQLite (`data/cehalon_items.db`), sync del JSON de `warframe-items` (release latest de WFCD).
  - Tool `buscar_item(nombre)` real: búsqueda exacta → difusa (top 5), formatea para LLM con stats, crafting, ingredientes.
  - *Done:* "¿qué es Soma Prime?" → `buscar_item` → stats/crafting reales en la respuesta.
  - **Resultado:** `src/brain/items.rs` con esquema completo (unique_name PK, índices en name/type/category/tradable), `init_db`, `sync_wfcd` (descarga release latest de GitHub WFCD, upsert), `search_items`/`get_item_by_name`/`format_item_for_llm`. Conexión global `Arc<Mutex<Connection>>` en `tools.rs`. `tools::execute("buscar_item")` usa BD real. Sync al arrancar: si BD vacía, descarga ~4000 items de WFCD release.
  - **Verificación:** 15/15 tests unitarios + TUI arranca → "[items] BD vacía, sincronizando WFCD..." → descarga ~4000 items → panel WORLDSTATE con datos reales. Tool `buscar_item` disponible para el modelo.

- [x] **P10 — warframe.market + tool `consultar_precio`** ✅ *2026-11-26*
  - Fetch de precio, caché con TTL en SQLite, formato con ▲▼ si hay cambio.
  - *Done:* "¿cuánto vale Soma Prime?" → precio real citado `(fuente: warframe.market)`.
  - **Resultado:** `get_price`/`fetch_and_cache_price` en `items.rs`: TTL en tabla `prices`, órdenes visibles de usuarios `online`/`ingame`, media de sell/buy y `format_price_for_llm` para la respuesta del modelo.

### Bloque D — Personalidad y control

- [x] **P11 — Comandos TUI** ✅ *2026-11-26*
  - `/personality`, `/model`, `/status`, `/worldstate`, `/clear`, `/quit`, `/help` + Tab de ayuda.
  - *Done:* cada comando funciona y muestra feedback en el chat.
  - **Resultado:** Sistema de comandos en `main.rs`:
    - `/help` / `/?` — lista todos los comandos con descripción
    - `/quit` / `/exit` — salida limpia con mensaje de despedida
    - `/clear` — limpia historial de chat
    - `/status` — muestra personalidad, modelo, Ollama, worldstate, contador de mensajes
    - `/personality <ordis|suda|simaris>` — cambia personalidad (valida modelo en Ollama)
    - `/model [nombre]` — sin args lista modelos; con args cambia modelo (valida en Ollama)
    - `/worldstate` — fuerza refresh (avisar, el fetcher thread actualiza en su ciclo)
    - Tab — autocompletado ciclado de comandos (`/personality`, `/model`, `/status`, `/worldstate`, `/clear`, `/quit`, `/help`)
    - Manejo de comandos desconocidos con sugerencia `/help`
  - **Verificación:** 15/15 tests unitarios + TUI arranca vivo sin errores.

- [x] **P12 — Cambio de personalidad en caliente** ✅ *2026-11-26*
  - `/personality ordis|suda|simaris` → `ollama create` del Modelfile + recarga del modelo en uso.
  - *Done:* cambiar de Ordis a Simaris cambia el tono de la respuesta sin reiniciar.

- [x] **P13 — Diseño final + Fansite Kit integration** ✅ *2026-11-26*
  - Markdown liviano en respuestas (negritas, listas), spinner de pensando, paleta pulida.
  - **Header con logo ASCII del Fansite Kit** (Logos/ 29 archivos → `WarframeLogo_Black.png` → ASCII art header en `logo.rs`).
  - **Gráficos del Kit** (Images/ 1,588 + Warframes/ 150+ + Key Art/ 141) → sprites/bordes HUD en TUI via `logo.rs` (sprites ASCII, bordes HUD).
  - **Audio** (Audio/ 482 archivos) → feedback sonoro por personalidad usando `rodio` 0.19:
    - **Ordis** (20 audios): incoming call, alarm emergency, engage purge, "is it talking"
    - **Suda/Simaris** (Lotus 10 audios): incoming transmission, message arrived, "it is time Tenno"
    - **UI genérico** (Miscellaneous): alarm, cipher fail, energy pickup, ship pager, single drum hit
    - Thread `rodio` 0.19 no bloqueante, canal `mpsc` desde event loop
    - Mapeo completo en `docs/FANSITE_KIT_AUDIO.md` (creado)
  - *Done:* TUI con branding visual + audio feedback coherente por personalidad.

### Bloque E — Cierre de fase

- [x] **P14 — `.gitignore`, README quickstart, tests** ✅ *2026-11-26*
  - README: requisitos, arranque, comandos. Tests unitarios de config y parseo de tools.
  - *Done:* `cargo test` verde + `cargo build --release` compila.

### Bloque F — Datos verificados y QA

- [x] **P15 — Tool `donde_encontrar` + prefetch por intención** ✅ *2026-11-26*
  - Tool con drops + probabilidad, polaridades, mods compatibles y receta; `src/brain/intent.rs` detecta la intención y ejecuta la tool antes de responder.
  - *Done:* "¿Dónde dropea Serration?" responde con fuentes reales y probabilidad; un item ausente del dataset se marca `ITEM NO CATALOGADO`.
  - **Resultado:** `tools.rs` con `donde_encontrar` + formateadores; `intent.rs` con `detect_all`/`unknown_items`; guard anti-alucinación en el bloque de prefetch de `main.rs`; flags `--tool`, `--ask` y `--model`.

- [x] **P16 — Suite de QA de datos (`src/qa.rs`)** ✅ *2026-11-26*
  - Batería de checks sobre el dataset WFCD: warframes, mods, drops, probabilidades e integridad con upstream.
  - *Done:* `cargo run -- --qa` → `RESULTADO QA: ✅ PASS` (27 checks, 0 fallidos).
  - **Resultado:** `--qa` mide la BD local; `--qa --sync` sincroniza WFCD antes de medir. Detectó y corrigió el bug de probabilidades al 860 % (chance en fracción vs porcentaje).

- [x] **P17 — QA end-to-end con Ollama + paso en CI** ✅ *2026-11-27*
  - Script `scripts/qa_ollama.ps1` con preguntas reales y checks positivos/negativos; paso de QA en GitHub Actions.
  - *Done:* `pwsh -File scripts/qa_ollama.ps1` → 5/5; CI verde con el paso de QA de datos.
  - **Resultado:** checks con semántica `esperar`/`opcional`/`prohibido` para tolerar la varianza del modelo 3B; en CI el paso corre con `continue-on-error: true` (informativo, no gate).

---

## Registro de cambios

| Fecha | Paso | Nota |
|-------|------|------|
| 2026-11-25 | P01 | ✅ Hecho. Deps cargadas, build OK. Limpieza accidental en repositorios_dudas. |
| 2026-11-25 | P02 | ✅ Hecho. config loader + 3 tests verdes. |
| 2026-11-25 | P03 | ✅ Hecho. Cliente Ollama, 5/5 tests, lista modelos reales. |
| 2026-11-26 | P04 | ✅ Hecho. TUI skeleton + event loop, 7/7 tests. Ollama caído y relanzado (12434). |
| 2026-11-26 | P05 | ✅ Hecho. Chat end-to-end (E2E Ordis 16.7s), 9/9 tests. Prueba interactiva `cargo run` pendiente del usuario. |
| 2026-11-26 | P06 | ✅ Hecho. Streaming por thread+mpsc, E2E >1 fragmento (32.7s), input no bloquea. |
| 2026-11-26 | P07 | ✅ Hecho. 4 tools + loop completo, E2E worldstate mock (33s), línea verde en TUI. |
| 2026-11-26 | P08 | ✅ Hecho. Worldstate real (warframestat.us) + thread fetcher, panel WORLDSTATE vivo, 15/15 tests. |
| 2026-11-26 | P09 | ✅ Hecho. SQLite + WFCD sync (~4000 items) + tool `buscar_item` real, TUI sincroniza al arrancar. |
| 2026-11-26 | P10 | ✅ Hecho. warframe.market + tool `consultar_precio` real, caché TTL SQLite. |
| 2026-11-26 | P11 | ✅ Hecho. 7 comandos TUI + Tab autocompletado, validación en Ollama. |
| 2026-11-26 | P12 | ✅ Hecho. Cambio personalidad en caliente (auto-create + reload), 19/19 tests. |
| 2026-11-26 | P13 | ✅ Hecho. Logo ASCII + sprites HUD + audio rodio thread, TUI vivo, 19/19 tests. |
| 2026-11-26 | P14 | ✅ Hecho. `.gitignore`, README completo y tests de config/tools. |
| 2026-11-26 | P15 | ✅ Hecho. Tool `donde_encontrar` + prefetch por intención; parser WFCD por 26 categorías (upstream ya no publica `items.json`); normalización de probabilidades a 0-100 %. |
| 2026-11-26 | P16 | ✅ Hecho. `src/qa.rs` con 27 checks, `--qa` / `--qa --sync` en verde. |
| 2026-11-27 | P17 | ✅ Hecho. `scripts/qa_ollama.ps1` 5/5 (también con Suda) + paso de QA en CI. |

## Pasos FIX (abiertos si algo se rompe después de tachado)

_(vacío)_

## Estado final de la Fase 1

Todos los pasos P01–P17 están tachados. La Fase 1 se cerró con la release
**v1.0.0** (2026-11-26): `cargo test` en verde (56 tests + 6 ignorados), QA de
datos PASS y CI verde en GitHub Actions.

# Changelog

Todas las novedades de Cephalon Mate.
El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el
proyecto usa [semver](https://semver.org/lang/es/).

## [1.0.0] — 2026-11-26

Primera versión estable del asistente.

### Agregado

- TUI en Rust con `ratatui`: header, panel de worldstate, chat con streaming,
  input con autocompletado (Tab) e historial (↑/↓).
- Layout adaptable: el header muestra el detalle completo en terminales de
  ≥100 columnas y se simplifica por debajo; los paneles pasan de columnas a
  apilados bajo 90 columnas.
- Cliente de Ollama con streaming de tokens y loop de tool-calling
  (máx. 4 rondas por mensaje).
- Tools del asistente: `worldstate`, `buscar_item`, `donde_encontrar`,
  `consultar_precio` y `buscar_guia` (stub declarado como tal).
- La tool `donde_encontrar` devuelve fuentes de drop con su probabilidad,
  polaridades de un warframe, mods compatibles según su polaridad y receta de
  crafteo con ingredientes en nombres legibles.
- Suite de QA de datos en `src/qa.rs` (27 checks en esta release), ejecutable con `--qa`;
  `--qa --sync` sincroniza WFCD antes de medir.
- Flag `--tool <nombre> [json]` para ejecutar una tool y ver su salida cruda.
- Flag `--ask "<pregunta>"` para consultar al modelo en modo headless, sin TUI.
- Flag `--model <nombre>` para sobrescribir el modelo de Ollama en `--ask`
  (permite pasar la QA end-to-end por distintas personalidades).
- Prefetch por intención anti-alucinación (`src/brain/intent.rs`): detecta la
  intención de la pregunta y ejecuta la tool adecuada antes de responder.
- Script `scripts/qa_ollama.ps1` para QA end-to-end con Ollama real y las
  tools de la app.
- Personalidades **Ordis**, **Suda** y **Simaris** con Modelfile propio en
  `modelfiles/`, creación automática del modelo en Ollama y trato configurable
  al usuario (Tenno / Viajero / Operador).
- Base de datos de items en SQLite sincronizada con **WFCD**
  (`warframe-items`): descarga en background, progreso visible en la UI y
  reintentos con back-off exponencial (250 ms → 4 s, 5 intentos).
- Sincronización del **worldstate** en background desde api.warframestat.us,
  con refresco configurable y comando `/worldstate` para forzarla.
- **Audio por NPC**: Ordis usa su propio set de sonidos y Suda/Simaris el de
  Lotus, con mapeo por tipo de evento (mensaje, espera, recepción, alerta).
  Reproducción en thread aparte vía `rodio`.
- Comandos de TUI: `/personality`, `/model`, `/status`, `/worldstate`, `/sync`,
  `/clear`, `/help`, `/quit`.
- Modo `--smoke` para chequeos de arranque sin TUI (lo usa CI).
- CI en GitHub Actions: build, tests, build release, smoke test y un paso
  informativo de QA de datos (`--qa --sync`) que no bloquea el release si falla
  por problemas de red.

### Cambiado

- La sincronización inicial de WFCD ya no bloquea el arranque: la UI aparece
  de inmediato y la descarga ocurre en segundo plano.
- La base de datos global de items se comparte entre el sync y las tools
  (`items::get_items_db`), liberando el lock durante la descarga.
- La resolución de la carpeta `Audio/` se hace en runtime (`audio_dir()`),
  soportando el kit junto al ejecutable o dentro del proyecto.
- La BD de items crece con las columnas `drops`, `polarities`, `stats`,
  `abilities`, `passive` y `market_cost`; las bases antiguas migran
  automáticamente al arrancar.
- El parser de WFCD descarga los JSON por categoría (26 categorías), ya que
  upstream ya no publica el `items.json` agregado.
- Los Modelfiles de Ordis, Suda y Simaris exigen consultar las tools para
  obtener datos de items, drops, polaridades y precios antes de responder.

### Corregido

- El panel de worldstate se repintaba inconsistente al cambiar el tamaño de la
  terminal; ahora el layout se recalcula en cada frame.
- El stream del modelo podía dejar el spinner colgado si se agotaban las
  rondas de herramientas.
- Se normalizan las probabilidades de drop de WFCD a porcentaje (0-100 %),
  corrigiendo el caso que mostraba valores como `860 %`.

[1.0.0]: https://github.com/FabianGallardo-coder/Cephalon_Mate/releases/tag/v1.0.0

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
- Tools reales: `worldstate`, `buscar_item`, `consultar_precio` y `buscar_guia`
  (stub declarado como tal).
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
- CI en GitHub Actions: build, tests, build release y smoke test.

### Cambiado

- La sincronización inicial de WFCD ya no bloquea el arranque: la UI aparece
  de inmediato y la descarga ocurre en segundo plano.
- La base de datos global de items se comparte entre el sync y las tools
  (`items::get_items_db`), liberando el lock durante la descarga.
- La resolución de la carpeta `Audio/` se hace en runtime (`audio_dir()`),
  soportando el kit junto al ejecutable o dentro del proyecto.

### Corregido

- El panel de worldstate se repintaba inconsistente al cambiar el tamaño de la
  terminal; ahora el layout se recalcula en cada frame.
- El stream del modelo podía dejar el spinner colgado si se agotaban las
  rondas de herramientas.

[1.0.0]: https://github.com/FabianGallardo-coder/Cephalon_Mate/releases/tag/v1.0.0

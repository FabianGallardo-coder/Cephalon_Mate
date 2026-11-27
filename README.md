# ⚡ Cephalon Mate

> Tu Cephalon personal con IA local — asistente de Warframe que corre 100% en tu máquina.

Cephalon Mate es una TUI en **Rust** que usa **Ollama** (IA local, sin nube) para
responder consultas de Warframe: estado en vivo del juego, base de datos de items,
precios de warframe.market y consultas normales con la personalidad que elijas.

## ¿Qué es?

- 🌐 **Info en vivo** — fissuras, alertas, sortie e invasiones (api.warframestat.us).
- 📦 **Base de datos de items** — sincronizada con WFCD (`warframe-items`) en background.
- 💰 **Precios** — consulta de warframe.market con caché en SQLite.
- 🎭 **Personalidad configurable** — **Ordis** (gracioso, desbordado), **Suda** (curiosa, lógica)
  o **Simaris** (inquisitiva, intensa). Cada una tiene su Modelfile en `modelfiles/`.
- 🗣️ **Te llama como quieras** — Tenno, Viajero u Operador (configurable).
- 🔊 **Audio por NPC** — Ordis suena con su voz, Suda/Simaris con la de Lotus.

## Arquitectura

```
TUI (ratatui)
 ├── cliente Ollama (Modelfile de personalidad + tools)
 ├── thread worldstate (api.warframestat.us, refresh 60s)
 ├── thread WFCD (descarga JSON por categoría → SQLite, con reintentos)
 └── thread de audio (rodio, sonidos del Fansite Kit)
```

## Requisitos

- **Rust 1.85+** (el crate usa edition 2024) — `cargo --version`
- **Git**
- **[Ollama](https://ollama.com)** — opcional pero recomendado; sin él el chat no responde,
  pero el resto de la app (worldstate, WFCD, audio) funciona igual.
  En este PC Ollama corre en el puerto `12434` (ver `docs/MODELOS.md`).
- **Audio** (opcional): la carpeta `Audio/` del Warframe Fansite Kit. Si no está, la app
  avisa por consola y sigue funcionando sin sonido.

## Instalación

```bash
git clone https://github.com/FabianGallardo-coder/Cephalon_Mate.git
cd Cephalon_Mate
cargo build --release
```

El binario queda en `target/release/cehalon-mate.exe` (Windows) o
`target/release/cehalon-mate` (Linux/macOS).

### Fansite Kit (audio)

El kit es de Digital Extremes y **no** se incluye en el repo (pesa ~1,7 GB). Para tener
sonido, copia la carpeta `Audio/` del kit:

```bash
# Windows
xcopy "Warframe Fansite Kit\Audio" "Audio\" /E /I
```

`audio_dir()` busca la carpeta en este orden:

1. `Audio/` junto al ejecutable
2. `Warframe Fansite Kit/Audio` junto al ejecutable
3. `Audio/` o `Warframe Fansite Kit/Audio` subiendo desde el directorio actual

## Uso

```bash
cargo run --release
# o
./target/release/cehalon-mate.exe
```

### Comandos del TUI

| Comando | Qué hace |
|---|---|
| `/personality <ordis\|suda\|simaris>` | Cambia la personalidad (crea el modelo de Ollama si no existe) |
| `/model <nombre>` | Cambia el modelo base de Ollama (ej. `qwen2.5:3b`) |
| `/status` | Estado del sistema: personalidad, modelo, Ollama, worldstate |
| `/worldstate` | Fuerza una actualización del worldstate (en background) |
| `/sync` | Re-sincroniza la BD de items con WFCD |
| `/clear` | Limpia el historial del chat |
| `/credits` | Muestra créditos y atribución a terceros (Digital Extremes, WFCD, warframe.market, warframestat.us, Ollama) |
| `/help` | Lista de comandos |
| `/quit` | Sale de la app |

Atajos: `Tab` autocompleta comandos, `↑`/`↓` recorren el historial, `Ctrl+C` sale.

### Flags de CLI

| Flag | Qué hace |
|---|---|
| `--qa` | Ejecuta la batería de QA de datos WFCD. |
| `--qa --sync` | Sincroniza la BD con WFCD y después ejecuta la batería de QA. |
| `--tool <nombre> [json]` | Ejecuta una tool y muestra su salida cruda; útil para depurar. |
| `--ask "<pregunta>"` | Pregunta al modelo en modo headless, sin TUI; es la vía de QA end-to-end. |
| `--model <nombre>` | Sobrescribe el modelo de Ollama usado por `--ask` (p. ej. `cehalon-suda:latest`). |
| `--version` | Muestra la versión. |
| `--smoke` | Chequeo de arranque sin TUI (lo usa CI). |
| `--help` | Muestra la ayuda. |

```bash
cargo run -- --qa
cargo run -- --qa --sync
cargo run -- --tool donde_encontrar '{"nombre":"Serration"}'
cargo run -- --ask "¿Dónde dropea Serration?"
```

### Tools del asistente

| Tool | Para qué sirve |
|---|---|
| `buscar_item` | Busca un item por nombre, con coincidencia exacta o difusa, y devuelve sus datos principales. |
| `donde_encontrar` | **Tool principal de obtención:** fuentes de drop, polaridades, mods compatibles y receta de crafteo. |
| `consultar_precio` | Consulta el precio de un item por su slug de warframe.market. |
| `worldstate` | Devuelve el estado en vivo: fissuras, alertas, sortie e invasiones. |
| `buscar_guia` | Stub declarado: la base de guías indexadas todavía no tiene implementación real. |

**`donde_encontrar`** devuelve dónde dropea un mod o item —enemigos, relíquias y
misiones, con su probabilidad—, las polaridades de un warframe, los mods
compatibles con ese warframe según su polaridad y la receta de crafteo con los
ingredientes expresados en nombres legibles.

### Datos verificados y anti-alucinación

Antes de responder, la app detecta la intención de la pregunta en
`src/brain/intent.rs`. Si pide información de un item, drop, stats,
polaridades o mods compatibles, ejecuta la tool correspondiente e inyecta al
modelo su salida como bloque de **datos verificados de WFCD**.

Los modelos de 3B no siempre llaman a las tools por su cuenta. Sin este
prefetch, podían alucinar polaridades y recetas; con él, responden con los
datos de la base local y distinguen lo verificado de su conocimiento general.

Si el item no existe en el dataset —por ejemplo, **Hornet Prime**, que
upstream WFCD ya no publica—, la app lo marca como `ITEM NO CATALOGADO` y
exige que el modelo lo diga con honestidad en vez de inventar drops, stats o
recetas.

## Audio

El set de sonidos depende de la **personalidad activa**, que mapea a un NPC del Fansite Kit:

| Personalidad | NPC | Carpeta |
|---|---|---|
| Ordis | Ordis | `Audio/NPCs/Ordis/` |
| Suda | Lotus | `Audio/NPCs/Lotus/` |
| Simaris | Lotus | `Audio/NPCs/Lotus/` |

Eventos y sonido que disparan:

| Evento | Ordis | Lotus (Suda/Simaris) |
|---|---|---|
| Mensaje del Cephalon | `Ordis - Incoming Call 1.mp3` | `Lotus_Incoming Communication.mp3` |
| Esperando respuesta | `Ordis - Incoming Call 2.mp3` | `Lotus_Message Waiting.mp3` |
| Tool ejecutada (datos recibidos) | `Ordis - Incoming Message 2.mp3` | `Lotus_Message Recieved.mp3` |
| Respuesta lista | `Ordis - Incoming Message 1.mp3` | `Lotus_Message Arrived.mp3` |
| Worldstate actualizado | `Ordis - Incoming Call or Message.mp3` | `Lotus_Transmission Recieved.mp3` |
| Fissura Steel Path (alerta crítica) | `Ordis - Alarm (System in Need).mp3` | `Lotus_It Is Time Tenno.mp3` |
| Alertas activas al refrescar | `Ordis - Alarm (Emergency).mp3` | `Lotus_Somethings Happening Tenno.mp3` |
| Error | `Miscellaneous/CipherFail.mp3` | `Miscellaneous/CipherFail.mp3` |

El audio corre en su propio thread (`mpsc`), así que nunca bloquea la UI.

## WFCD (base de datos de items)

Upstream [WFCD/warframe-items](https://github.com/WFCD/warframe-items) ya no
publica un `items.json` agregado. La app descarga los JSON por categoría (26
categorías) desde:

```text
https://raw.githubusercontent.com/WFCD/warframe-items/master/data/json/<Categoría>.json
```

- El sync corre en un thread aparte con **back-off exponencial**: 250 ms,
  500 ms, 1 s, 2 s… (5 intentos máximo). Si falla, la app sigue viva y
  reintenta con `/sync`.
- La descarga se hace **sin** bloquear la BD: mientras bajan los JSON, las
  tools siguen respondiendo con los datos que ya hay en SQLite.
- Se guarda en `data/cehalon_items.db` (SQLite, ignorado por git).
- El dataset sincronizado contiene 20.166 items; las fuentes de drop cubren
  aproximadamente el 24 % de ellos.

La BD incluye estas columnas nuevas, añadidas automáticamente al migrar una
base de datos antigua:

| Columna | Contenido |
|---|---|
| `drops` | Fuentes de drop de WFCD, con lugar, tipo, probabilidad y rareza. |
| `polarities` | Polaridades de cada warframe. |
| `stats` | Stats de combate resumidas. |
| `abilities` | Habilidades del warframe. |
| `passive` | Descripción de la pasiva. |
| `market_cost` | Rango de precio orientativo publicado por WFCD. |

WFCD expresa `chance` en fracción para enemigos (`0.22` = 22 %) y en
porcentaje para relíquias (`8.6` = 8.6 %). La app normaliza ambos formatos a
porcentaje, siempre en el rango 0-100 %.

El worldstate también va en background: se refresca cada `worldstate_refresh_secs`
(60 s por defecto) desde `api.warframestat.us`.

## Configuración

`config.toml` (se crea solo con defaults si no existe):

```toml
personality = "ordis"          # ordis | suda | simaris
user_address = "Tenno"         # Tenno | Viajero | Operador
model = "qwen2.5:3b"           # modelo base de Ollama
ollama_host = "http://127.0.0.1:12434"
worldstate_refresh_secs = 60
```

## Pruebas

Los tests unitarios se ejecutan con:

```bash
cargo test
```

### QA de datos WFCD

```bash
cargo run -- --qa
cargo run -- --qa --sync
```

`--qa` comprueba el dataset local sin volver a descargarlo; `--qa --sync`
sincroniza primero con WFCD y después ejecuta los mismos checks.

| Check | Qué verifica |
|---|---|
| BD poblada | Que la base local contiene items. |
| Cobertura de drops | Cuántos items tienen al menos una fuente de drop. |
| Warframes | La información de 8 warframes de referencia: polaridades, stats y habilidades. |
| Mods | La polaridad y las fuentes de drop de 7 mods de referencia. |
| Mods compatibles | Que se listan mods compatibles con las polaridades de Rhino. |
| Búsqueda difusa | Que una búsqueda parcial, como `soma`, devuelve resultados. |
| Salida de `donde_encontrar` | Que la respuesta de la tool contiene los datos esperados. |
| Parseo de drops | Que las fuentes de drop guardadas tienen una ubicación legible. |
| Integridad con upstream | Que la BD local coincide con los JSON de WFCD. |
| Probabilidades | Que todas las probabilidades están normalizadas entre 0 y 100 %. |
| Item desconocido | Que un item ausente se marca como no catalogado y no se inventa. |
| Prefetch por intención | Que la pregunta se encamina a la tool correcta. |

### QA end-to-end con Ollama

```bash
pwsh -File scripts/qa_ollama.ps1
pwsh -File scripts/qa_ollama.ps1 -Modelo "cehalon-suda:latest"
```

El script pregunta al modelo con Ollama real, ejecuta las tools por el flujo de
la app y comprueba que la respuesta usa los datos de WFCD. Cada pregunta tiene
textos que **deben** aparecer (datos reales) y textos que **no** deben aparecer
(alucinaciones conocidas, como inventar una receta para un item que no está en
el dataset). Requiere Ollama activo y el binario debug disponible.

Los tests que necesitan red u Ollama están marcados con `#[ignore]`:

```bash
cargo test -- --ignored
```

## CI

`.github/workflows/ci.yml` corre en cada push y PR sobre `main`, `master` y `Master`:

1. `cargo build`
2. `cargo test --verbose`
3. `cargo build --release`
4. Smoke test: `cehalon-mate --smoke` debe imprimir `✅ Worldstate actualizado`
5. Paso informativo de QA de datos: `cehalon-mate --qa --sync`.
   Si falla por problemas de red, no bloquea el release.

## Documentación

| Doc | Qué contiene |
|-----|--------------|
| [`PROYECTO.md`](PROYECTO.md) | Documento maestro: idea, decisiones, fases |
| [`PLAN_EJECUCION.md`](PLAN_EJECUCION.md) | Plan por fases con su estado |
| [`AGENTS.md`](AGENTS.md) | Reglas para agentes IA que trabajen en el repo |
| [`docs/ARQUITECTURA.md`](docs/ARQUITECTURA.md) | Diseño técnico por capas |
| [`docs/ESTRUCTURA.md`](docs/ESTRUCTURA.md) | Estructura de carpetas del repo |
| [`docs/PERSONALIDADES.md`](docs/PERSONALIDADES.md) | Ordis / Suda / Simaris y trato al usuario |
| [`docs/MODELOS.md`](docs/MODELOS.md) | Modelos Ollama por nivel de recursos |
| [`docs/DATOS_Y_REPOS.md`](docs/DATOS_Y_REPOS.md) | APIs, repos y fuentes de datos |
| [`docs/FANSITE_KIT_AUDIO.md`](docs/FANSITE_KIT_AUDIO.md) | Mapa de sonidos del Fansite Kit |

## Assets

Identidad visual y sonora del **Warframe Fansite Kit oficial** de Digital Extremes:
https://digitalextremesltd.app.box.com/s/vh2u41yhdlgp3girffucbynadoi33173

El kit no se distribuye con el repo. Sus audios son de Digital Extremes y se usan
para uso de fansite/comunidad.

## Licencia

MIT — ver [LICENSE](LICENSE). Los audios del Fansite Kit **no** están cubiertos por
esta licencia.

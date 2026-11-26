# ⚡ Cephalon Mate

> Tu Cephalon personal con IA local — asistente de Warframe que corre 100% en tu máquina.

Cephalon Mate es una TUI en **Rust** que usa **Ollama** (IA local, sin nube) para
responde consultas de Warframe: estado en vivo del juego, base de datos de items,
precios de warframe.market y consultas normales con la personalidad de tu choice.

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
 ├── thread WFCD (descarga items.json → SQLite, con reintentos)
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
| `/help` | Lista de comandos |
| `/quit` | Sale de la app |

Atajos: `Tab` autocompleta comandos, `↑`/`↓` recorren el historial, `Ctrl+C` sale.

### Flags de CLI

```bash
cehalon-mate --version   # versión
cehalon-mate --smoke     # chequeo de arranque sin TUI (lo usa CI)
cehalon-mate --help
```

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
| Mensaje recibido | `Ordis - Incoming Message 2.mp3` | `Lotus_Message Recieved.mp3` |
| Respuesta lista | `Ordis - Incoming Message 1.mp3` | `Lotus_Message Arrived.mp3` |
| Worldstate actualizado | `Ordis - Incoming Call or Message.mp3` | `Lotus_Transmission Recieved.mp3` |
| Alerta (Steel Path) | `Ordis - Alarm (Emergency).mp3` | `Lotus_Somethings Happening Tenno.mp3` |
| Error | `Miscellaneous/CipherFail.mp3` | `Miscellaneous/CipherFail.mp3` |

El audio corre en su propio thread (`mpsc`), así que nunca bloquea la UI.

## WFCD (base de datos de items)

- Descarga `items.json` de la release *latest* de [WFCD/warframe-items](https://github.com/WFCD/warframe-items).
- Corre en un thread aparte con **back-off exponencial**: 250 ms, 500 ms, 1 s, 2 s…
  (5 intentos máximo). Si falla, la app sigue viva y reintenta con `/sync`.
- La descarga se hace **sin** bloquear la BD: mientras baja el JSON, las tools siguen
  respondiendo con los datos que ya hay en SQLite.
- Se guarda en `data/cehalon_items.db` (SQLite, ignorado por git).

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

```bash
cargo test
```

Los tests que necesitan red u Ollama están marcados con `#[ignore]`:

```bash
cargo test -- --ignored
```

## CI

`.github/workflows/ci.yml` corre en cada push y PR sobre `main`/`master`:

1. `cargo build`
2. `cargo test --verbose`
3. `cargo build --release`
4. Smoke test: `cehalon-mate --smoke` debe imprimir `✅ Worldstate actualizado`

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

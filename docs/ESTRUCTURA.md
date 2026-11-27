# Estructura del repo — Cephalon Mate

```
Cephalon_Mate/
├── README.md                 # Visión general y arranque
├── CHANGELOG.md              # Novedades por versión (Keep a Changelog)
├── PROYECTO.md               # ⭐ Documento maestro (leer primero)
├── AGENTS.md                 # Reglas para agentes IA
├── PLAN_EJECUCION.md         # Plan por fases con su estado (P01–P17)
├── LICENSE                   # MIT
├── .gitignore                # target/, data/, Warframe Fansite Kit*
├── .github/
│   └── workflows/ci.yml      # CI: build + tests + release + smoke + QA de datos
│
├── docs/
│   ├── ARQUITECTURA.md       # Diseño por capas, decisiones técnicas, flujo
│   ├── ESTRUCTURA.md         # Este archivo
│   ├── PERSONALIDADES.md     # Ordis/Suda/Simaris + trato al usuario
│   ├── MODELOS.md            # Modelos Ollama por recursos
│   ├── DATOS_Y_REPOS.md      # APIs, repos, assets, fuentes
│   └── FANSITE_KIT_AUDIO.md  # Catálogo y mapeo de sonidos del kit
│
├── src/
│   ├── main.rs               # Entry point: flags CLI, event loop, comandos TUI, prefetch
│   ├── ui.rs                 # Render ratatui (header, worldstate, chat, input)
│   ├── config.rs             # Carga/guardado de config.toml con defaults
│   ├── qa.rs                 # Suite de QA de datos (flags --qa / --qa --sync)
│   ├── audio.rs              # Thread de audio con rodio + mapeo NPC/evento
│   ├── logo.rs               # Logo ASCII y sprites HUD del Fansite Kit
│   └── brain/
│       ├── mod.rs            # Re-exportos del cerebro
│       ├── client.rs         # Cliente Ollama (ureq, streaming, tool-calling)
│       ├── chat.rs           # Tipos de mensaje/request/response de Ollama
│       ├── tools.rs          # Las 5 tools expuestas al modelo + formateadores
│       ├── intent.rs         # Detección de intención y prefetch anti-alucinación
│       ├── items.rs          # WFCD (26 categorías) → SQLite + warframe.market
│       └── worldstate.rs     # Fetcher en background de api.warframestat.us
│
├── modelfiles/               # Modelfiles por personalidad (texto, se commitean)
│   ├── ordis.Modelfile
│   ├── suda.Modelfile
│   └── simaris.Modelfile
│
├── scripts/
│   └── qa_ollama.ps1         # QA end-to-end con Ollama real (5 preguntas)
│
├── config.toml               # personality, user_address, model, refresh rates
├── Cargo.toml / Cargo.lock
│
├── Warframe Fansite Kit/     # ✅ Assets oficiales DE (2.320 archivos) — ignorado en git
│   ├── Audio/                #    482 archivos — voces/frases de Cephalon
│   ├── Logos/                #     29 archivos — logo para TUI/header
│   ├── Images/               #  1.588 archivos — sprites, íconos
│   ├── Key Art/              #    141 archivos — arte promocional/fondo
│   ├── Concept Art/          #     19 archivos — referencia visual
│   ├── Content Creation/     #     54 archivos — plantillas
│   └── Languages/            #      7 archivos — localización
│
└── data/                     # Runtime: cehalon_items.db — ignorado en git
    └── (cehalon_items.db)
```

## Convenciones

- `data/`, `target/` y `Warframe Fansite Kit*` **no** se commitean (binarios pesados, datos locales).
- Los `modelfiles/` sí se commitean (son texto, son parte del comportamiento).
- `config.toml` se commitea con defaults; cada usuario puede editarlo localmente.
- Todo doc nuevo va en `docs/` salvo README/CHANGELOG/PROYECTO/AGENTS/PLAN_EJECUCION, que viven en la raíz.

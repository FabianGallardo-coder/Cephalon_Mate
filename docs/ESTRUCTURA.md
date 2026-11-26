# Estructura del repo — Cephalon Mate

```
Cephalon_Mate/
├── README.md                 # Visión general y arranque
├── PROYECTO.md               # ⭐ Documento maestro (leer primero)
├── AGENTS.md                 # Reglas para agentes IA
├── .gitignore                # target/, data/*.db, assets/ descargados
│
├── docs/
│   ├── ARQUITECTURA.md       # Diseño por capas, decisiones técnicas, flujo
│   ├── ESTRUCTURA.md         # Este archivo
│   ├── PERSONALIDADES.md     # Ordis/Suda/Simaris + trato al usuario
│   ├── MODELOS.md            # Modelos Ollama por recursos
│   └── DATOS_Y_REPOS.md      # APIs, repos, assets, fuentes
│
├── src/                      # ⬜ Fase 1 — código Rust (aún no existe)
│   ├── main.rs               # Entry point + arranque TUI
│   ├── tui/                  # ratatui: layout, chat, input, status
│   ├── brain/                # Cliente Ollama, manejo de tools, streaming
│   ├── tools/                # buscar_item, consultar_precio, worldstate, buscar_guia
│   ├── db/                   # esquema SQLite, queries, sync de datos
│   └── config.rs             # carga de config.toml
│
├── modelfiles/               # Modelfiles por personalidad
│   ├── ordis.Modelfile
│   ├── suda.Modelfile
│   └── simaris.Modelfile
│
├── config.toml               # personality, user_address, model, refresh rates
│
├── Warframe Fansite Kit/     # ✅ Assets oficiales DE (2.320 archivos) — ignorado en git
│   ├── Audio/                #    482 archivos — voces/frases de Cephalon
│   ├── Logos/                #    29 archivos — logo para TUI/header
│   ├── Images/               #  1.588 archivos — sprites, íconos
│   ├── Key Art/              #    141 archivos — arte promocional/fondo
│   ├── Concept Art/          #     19 archivos — referencia visual
│   ├── Content Creation/     #     54 archivos — plantillas
│   └── Languages/            #      7 archivos — localización
│
├── data/                     # ⬜ Runtime: cehalon.db, knowledge base — ignorado en git
│   └── (cehalon.db)
│
└── Cargo.toml                # ⬜ Fase 1
```

## Convenciones

- `data/` y `assets/` **no** se commitean (binarios pesados, datos locales).
- Los `modelfiles/` sí se commitean (son texto, son parte del comportamiento).
- `config.toml` se commitea con defaults; cada usuario puede editarlo localmente.
- Todo doc nuevo va en `docs/` salvo README/PROYECTO/AGENTS que viven en la raíz.

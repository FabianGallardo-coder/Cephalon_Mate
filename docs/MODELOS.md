# Modelos — Cephalon Mate

**Criterio del proyecto: universal y de bajos recursos.** El default debe correr en cualquier equipo razonable (8 GB RAM, sin GPU dedicada), con calidad suficiente para chat + tool calling.

## Default: `qwen2.5:3b`

Por qué:

- **Multilingüe real** — el usuario habla español; qwen2.5 responde mucho mejor en español que llama3.2 pequeño o gemma.
- **Tamaño ideal** — ~1,9 GB de descarga, corre cómodo en CPU con 8 GB de RAM, tokens/s aceptables sin GPU.
- **Tool calling decente** — soporta la API de tools de Ollama, que es el corazón de las consultas de datos.
- Ya disponible en el entorno: `qwen2.5:7b`, `qwen2.5-coder`, `llama3.1:8b`, `nomic-embed-text`.

## Tabla por nivel de recursos

| Nivel | Modelo | RAM aprox. | Cuándo |
|-------|--------|-----------|--------|
| 🟢 Mínimo | `qwen2.5:1.5b` | ~2 GB | PCs muy justos, modo rápido |
| 🔵 **Default** | **`qwen2.5:3b`** | **~4 GB** | **Cualquier equipo moderno** |
| 🟡 Alta | `qwen2.5:7b` | ~6 GB | 16 GB de RAM o con GPU |
| 🟡 Alternativa | `llama3.1:8b` | ~6 GB | Si se prefiere estilo conversacional |
| ⚪ Embeddings | `nomic-embed-text` | ~0,3 GB | Siempre (RAG, Fase 2) |

## Reglas

1. El modelo base es **configurable** en `config.toml → model`; `/model` sin argumentos lista los modelos instalados y con argumento cambia en vivo (rechaza con la lista de disponibles si el modelo pedido no está instalado).
2. Las personalidades (Modelfiles) se **derivan** del modelo base — no duplican pesos, solo system prompt + parámetros.
3. `num_ctx = 4096` por default (suficiente para tools + historial corto; subirlo dispara el uso de RAM).
4. `temperature`: 0.7 para Ordis/Suda, 0.5 para Simaris (más severo y literal).

## Verificación en el entorno

```powershell
ollama list                      # modelos instalados
ollama pull qwen2.5:3b           # si falta el default
ollama show qwen2.5:3b           # ¿soporta tools?
```

Un modelo sirve para el proyecto si `ollama show` lista `tools` en capabilities.

## ⚠️ Gotcha del entorno: puerto 11434 bloqueado (2026-11-25)

**Síntoma:** `ollama serve` muere con
`listen tcp 0.0.0.0:11434: bind: An attempt was made to access a socket in a way forbidden by its access permissions`.

**Causa:** Windows/Hyper-V reservó el rango `11404–11503` (excluded port range), que se traga el 11434 de Ollama. Verificar con:
```powershell
netsh interface ipv4 show excludedportrange protocol=tcp
```

**Solución usada en este entorno:** arrancar el server en un puerto libre (12434):
```powershell
$env:OLLAMA_HOST = "127.0.0.1:12434"
ollama serve
```
El cliente también necesita `OLLAMA_HOST` apuntando a ese puerto.

**Solución permanente APLICADA (2026-11-25):** variable de entorno de usuario
`OLLAMA_HOST=http://127.0.0.1:12434` guardada en el registro + app oficial de
Ollama reiniciada en ese puerto. Funciona sin reboot.

- ⚠️ Sesiones de shell **anteriores** al fix no heredan la variable: si `ollama` da
  error de conexión, hacer `$env:OLLAMA_HOST = "http://127.0.0.1:12434"` primero.
  Las terminales nuevas la traen solas.

**Modelos creados y verificados (2026-11-25):**
| Modelo | ID | Base |
|--------|-----|------|
| `cehalon-ordis` | 9a0c0b06f4a2 | qwen2.5:3b |
| `cehalon-suda` | 4749ff90fa55 | qwen2.5:3b |
| `cehalon-simaris` | cd987649aa98 | qwen2.5:3b |

Tests: Ordis y Simaris responden en español dirigiéndose "Tenno" ✅

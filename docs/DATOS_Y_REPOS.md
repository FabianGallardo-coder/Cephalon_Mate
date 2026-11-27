# Datos y Repos — Cephalon Mate

Todas las fuentes de datos y repos de apoyo. **Prohibido inventar datos fuera de esta lista.**

## APIs en vivo

| Fuente | URL | Uso | Frecuencia |
|--------|-----|-----|------------|
| Worldstate | `https://api.warframestat.us/pc/{fissures,alerts,sortie,invasions}` | Alertas, fissuras, invasiones, sorties | ~60 s (configurable) |
| warframe.market | `https://api.warframe.market/v1/items` | Precios de venta/compra de items | TTL ~5 min por item |
| WFCD `warframe-items` | `https://raw.githubusercontent.com/WFCD/warframe-items/master/data/json/<Categoría>.json` (26 categorías; ver `docs/ARQUITECTURA.md`) | Items completos: crafting, stats, drops, polaridades | Sync al arrancar, con `/sync` para repetir |
| WFCD `warframe-worldstate-data` | GitHub WFCD | Metadatos de worldstate (nombres, tipos) | Sync semanal |

## Repos clave (WFCD — Warframe Community Developers)

| Repo | Qué aporta |
|------|-----------|
| [WFCD/warframe-items](https://github.com/WFCD/warframe-items) | ⭐ 435★ — Base de items oficial comunitaria. "All Warframe items from the official API" |
| [WFCD/warframe-worldstate-data](https://github.com/WFCD/warframe-worldstate-data) | Data del worldstate para parsear alertas/fissuras |
| [WFCD/warframe-worldstate-parser](https://github.com/WFCD/warframe-worldstate-parser) | Parser TS del worldstate (referencia de estructura; en Rust lo hacemos nosotros) |
| [WFCD/WFinfo](https://github.com/WFCD/WFinfo) | Referencia: cómo muestran precios/ducats de fissuras |
| [WFHelper/WFHelper](https://github.com/WFHelper/WFHelper) | Referencia: companion app (inventario, precios, mastery) |
| [akmayer/Warframe-Algo-Trader](https://github.com/akmayer/Warframe-Algo-Trader) | Referencia: lógica de trading con warframe.market |
| [WFCD/warframe-hub](https://github.com/WFCD/warframe-hub) | Referencia de datos comunitarios |

## Conocimiento del juego (Fase 2 — RAG)

Fuentes a indexar (documentación a confirmar licencias antes de indexar):

- Descripciones y lore incluidos en `warframe-items` (ya bajo licencia de uso de datos abierta).
- Datos extraídos: [macdja38/WarframeData](https://github.com/macdja38/WarframeData) (2016, sin actualizar — verificar vigencia).
- Wikis WFCD (warframe.fandom.com / wiki comunitaria) — scrapear solo si la licencia lo permite; alternativa: guías propias curadas en `data/kb/`.

## Assets — Warframe Fansite Kit (Digital Extremes)

**Descargado localmente (2026-11-25):**
```
C:\Users\Fabian\Desktop\_Proyectos\Cephalon_Mate\Warframe Fansite Kit
```
Contenido verificado: `Audio` (482), `Concept Art` (19), `Content Creation` (54), `Images` (1.588), `Key Art` (141), `Languages` (7), `Logos` (29) = 2.320 archivos.

Link original: https://digitalextremesltd.app.box.com/s/vh2u41yhdlgp3girffucbynadoi33173

| Carpeta | Archivos | Tamaño | Uso en Cephalon Mate |
|---------|----------|--------|----------------------|
| **Audio** | 482 | 364 MB | ⭐ Voces/frases de Cephalon para la mascota |
| **Logos** | 29 | 6,7 MB | Logo en TUI/header |
| Images | 1.588 | 510 MB | Sprites e íconos (Fase 3) |
| Key Art | 141 | 757 MB | Fondo de la mascota |
| Content Creation | 54 | 37 MB | Plantillas |
| Concept Art | 19 | 29 MB | Referencia visual |
| Languages | 7 | 5 MB | Localización |

**Plan de descarga:** solo `Audio` + `Logos` primero (Box permite descarga por carpeta). Los 1,6 GB de arte se bajan solo si la Fase 3 los necesita.

**Licencia:** el kit es de Digital Extremes para uso de fansites/comunidad — no redistribuir los assets como propios ni usarlos con fines comerciales.

## APIs/Tools que NO usaremos

- Aimbots/cheats (DrNseven/*) — fuera de alcance, EULA.
- APIs que requieran key de pago.

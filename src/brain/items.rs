//! Base de datos de items de Warframe (WFCD) + tool `buscar_item`.
//!
//! P09 del `PLAN_EJECUCION.md`: SQLite con esquema `items`, sync del JSON
//! de `warframe-items` (release latest), y tool real `buscar_item`.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use urlencoding;

/// Path de la base de datos SQLite (en `data/` junto al config.toml).
pub fn db_path() -> std::path::PathBuf {
    Path::new("data").join("cehalon_items.db")
}

/// Conexión global a la BD de items (protegida por Mutex + Arc para compartir).
static ITEMS_DB: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// Inicializa la conexión global a la BD de items (se llama al arrancar).
pub fn set_items_db(conn: Connection) {
    let arc = Arc::new(Mutex::new(conn));
    ITEMS_DB.set(arc).ok();
}

/// Obtiene la conexión a la BD (para `execute` y para el sync en background).
pub fn get_items_db() -> Option<Arc<Mutex<Connection>>> {
    ITEMS_DB.get().cloned()
}

/// Inicializa la BD: crea tablas e índices si no existen.
pub fn init_db() -> Result<Connection, String> {
    std::fs::create_dir_all("data").map_err(|e| format!("crear data/: {e}"))?;
    let conn = Connection::open(db_path()).map_err(|e| format!("abrir BD: {e}"))?;
    create_schema(&conn)?;
    Ok(conn)
}

/// Crea el esquema (tablas e índices) en la conexión dada.
pub fn create_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS items (
            unique_name     TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            description     TEXT,
            type            TEXT,
            category        TEXT,
            subcategory     TEXT,
            rarity          TEXT,
            polarity        TEXT,
            mastery_req     INTEGER,
            tradable        INTEGER,
            patchlogs       TEXT,
            image_name      TEXT,
            thumb           TEXT,
            url_name        TEXT,
            wikia_url       TEXT,
            wiki_url        TEXT,
            "drop"          TEXT,
            build_price     INTEGER,
            build_quantity  INTEGER,
            build_time      INTEGER,
            ingredients     TEXT,      -- JSON array
            components      TEXT,      -- JSON array (para prime sets)
            drops           TEXT,      -- JSON array [{location,type,chance,rarity}]
            polarities      TEXT,      -- JSON array (polaridades de warframe)
            stats           TEXT,      -- JSON object (stats de combate resumidas)
            abilities       TEXT,      -- JSON array (habilidades de warframe)
            passive         TEXT,      -- descripción de pasiva
            market_cost     TEXT,      -- JSON object (rango de precio en platina)
            created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS prices (
            item_slug       TEXT PRIMARY KEY,      -- slug del item en warframe.market (ej. "soma_prime_set")
            item_name       TEXT NOT NULL,         -- nombre legible
            sell_min        INTEGER,               -- menor precio de venta (platinum)
            sell_max        INTEGER,               -- mayor precio de venta
            sell_avg        INTEGER,               -- precio medio de venta
            buy_max         INTEGER,               -- mayor precio de compra
            buy_min         INTEGER,               -- menor precio de compra
            buy_avg         INTEGER,               -- precio medio de compra
            volume_24h      INTEGER,               -- volumen de trades en 24h
            updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at      DATETIME               -- TTL: cache expira a esta hora
        );

        CREATE INDEX IF NOT EXISTS idx_items_name ON items(name);
        CREATE INDEX IF NOT EXISTS idx_items_type ON items(type);
        CREATE INDEX IF NOT EXISTS idx_items_category ON items(category);
        CREATE INDEX IF NOT EXISTS idx_items_tradable ON items(tradable);
        CREATE INDEX IF NOT EXISTS idx_prices_expires ON prices(expires_at);
        "#,
    )
    .map_err(|e| format!("crear tablas: {e}"))?;

    migrate_schema(conn)?;

    Ok(())
}

/// Añade las columnas nuevas a una BD creada por una versión anterior.
///
/// `CREATE TABLE IF NOT EXISTS` no modifica tablas existentes, así que sin esta
/// migración un `data/cehalon_items.db` viejo se quedaría sin `drops` y la
/// tool "dónde encontrarlo" no tendría nada que mostrar.
fn migrate_schema(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info('items')")
        .map_err(|e| format!("pragma: {e}"))?;
    let existing: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| format!("pragma query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let new_columns: [(&str, &str); 6] = [
        ("drops", "TEXT"),
        ("polarities", "TEXT"),
        ("stats", "TEXT"),
        ("abilities", "TEXT"),
        ("passive", "TEXT"),
        ("market_cost", "TEXT"),
    ];

    for (col, ty) in new_columns {
        if !existing.iter().any(|c| c == col) {
            conn.execute_batch(&format!("ALTER TABLE items ADD COLUMN {col} {ty}"))
                .map_err(|e| format!("ALTER TABLE items ADD COLUMN {col}: {e}"))?;
            println!("[items] migración: columna '{col}' añadida");
        }
    }
    Ok(())
}

/// Fila de item para consultas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRow {
    pub unique_name: String,
    pub name: String,
    pub description: Option<String>,
    pub item_type: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub rarity: Option<String>,
    pub polarity: Option<String>,
    pub mastery_req: Option<i64>,
    pub tradable: Option<i64>,
    pub image_name: Option<String>,
    pub build_price: Option<i64>,
    pub build_quantity: Option<i64>,
    pub build_time: Option<i64>,
    pub ingredients: Option<String>,  // JSON
    pub components: Option<String>,   // JSON
    pub wikia_url: Option<String>,
    pub drops: Option<String>,        // JSON array de fuentes de drop
    pub polarities: Option<String>,   // JSON array (warframes)
    pub stats: Option<String>,        // JSON object de stats de combate
    pub abilities: Option<String>,    // JSON array (warframes)
    pub passive: Option<String>,
    pub market_cost: Option<String>,  // JSON object de precio en platina
}

/// Columnas de `items` que se proyectan en las consultas, en orden.
const ITEM_COLUMNS: &str = "unique_name, name, description, type, category, subcategory, \
     rarity, polarity, mastery_req, tradable, image_name, \
     build_price, build_quantity, build_time, ingredients, components, wikia_url, \
     drops, polarities, stats, abilities, passive, market_cost";

/// Construye un [`ItemRow`] desde una fila de SQLite con el orden de
/// [`ITEM_COLUMNS`].
fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        unique_name: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        item_type: row.get(3)?,
        category: row.get(4)?,
        subcategory: row.get(5)?,
        rarity: row.get(6)?,
        polarity: row.get(7)?,
        mastery_req: row.get(8)?,
        tradable: row.get(9)?,
        image_name: row.get(10)?,
        build_price: row.get(11)?,
        build_quantity: row.get(12)?,
        build_time: row.get(13)?,
        ingredients: row.get(14)?,
        components: row.get(15)?,
        wikia_url: row.get(16)?,
        drops: row.get(17)?,
        polarities: row.get(18)?,
        stats: row.get(19)?,
        abilities: row.get(20)?,
        passive: row.get(21)?,
        market_cost: row.get(22)?,
    })
}

/// Busca items por nombre (LIKE case-insensitive) — tool `buscar_item`.
/// Escapa `%` y `_` para que actúen como literales, no como comodines.
pub fn search_items(conn: &Connection, query: &str, limit: usize) -> Result<Vec<ItemRow>, String> {
    let escaped = query.to_lowercase().replace(['%', '_'], "");
    let pattern = format!("%{}%", escaped);
    let prefix = format!("{}%", escaped);
    let sql = format!(
        r#"
        SELECT {ITEM_COLUMNS}
        FROM items
        WHERE lower(name) LIKE ?1 ESCAPE '\'
           OR lower(unique_name) LIKE ?1 ESCAPE '\'
        ORDER BY
            CASE WHEN lower(name) = ?2 THEN 0
                 WHEN lower(name) LIKE ?3 ESCAPE '\' THEN 1
                 ELSE 2 END,
            name
        LIMIT ?4
        "#
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;

    let rows = stmt
        .query_map(
            params![
                pattern,
                query.to_lowercase(),
                prefix,
                limit as i64
            ],
            row_to_item,
        )
        .map_err(|e| format!("query: {e}"))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("row: {e}"))?);
    }
    Ok(out)
}

/// Obtiene un item por nombre exacto (para tool `buscar_item` con nombre preciso).
pub fn get_item_by_name(conn: &Connection, name: &str) -> Result<Option<ItemRow>, String> {
    let sql = format!(
        r#"
        SELECT {ITEM_COLUMNS}
        FROM items
        WHERE lower(name) = ?1
           OR lower(unique_name) = ?1
        LIMIT 1
        "#
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;

    let row = stmt
        .query_row(params![name.to_lowercase()], row_to_item)
        .optional()
        .map_err(|e| format!("query: {e}"))?;

    Ok(row)
}

/// Cuenta total de items en la BD.
pub fn count_items(conn: &Connection) -> Result<usize, String> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0))
        .map_err(|e| format!("count: {e}"))?;
    Ok(count as usize)
}

/// Un item de Warframe del dataset de WFCD (`warframe-items`).
///
/// El parseo es a mano (`serde_json::Value`) y no con `derive(Deserialize)`:
/// upstream cambia el schema entre versiones y un campo raro no debe tumbar
/// el sync entero.
#[derive(Debug, Clone)]
struct WfcdItem {
    unique_name: String,
    name: String,
    description: Option<String>,
    item_type: Option<String>,
    category: Option<String>,
    subcategory: Option<String>,
    rarity: Option<String>,
    polarity: Option<String>,
    mastery_req: Option<i64>,
    tradable: Option<bool>,
    image_name: Option<String>,
    wikia_url: Option<String>,
    build_price: Option<i64>,
    build_quantity: Option<i64>,
    build_time: Option<i64>,
    components: Option<String>,
    drops: Option<String>,
    polarities: Option<String>,
    stats: Option<String>,
    abilities: Option<String>,
    passive: Option<String>,
    market_cost: Option<String>,
}

/// Campo string (o `None`).
fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(str::to_string)
}

/// Campo numérico tolerante (int, float o string numérica).
fn num_field(v: &serde_json::Value, key: &str) -> Option<i64> {
    let x = v.get(key)?;
    if let Some(i) = x.as_i64() {
        return Some(i);
    }
    if let Some(f) = x.as_f64() {
        return Some(f as i64);
    }
    if let Some(s) = x.as_str() {
        return s.parse::<f64>().ok().map(|f| f as i64);
    }
    None
}

/// Campo booleano tolerante.
fn bool_field(v: &serde_json::Value, key: &str) -> Option<bool> {
    match v.get(key)? {
        serde_json::Value::Bool(b) => Some(*b),
        serde_json::Value::String(s) => s.parse::<bool>().ok(),
        _ => None,
    }
}

/// Polaridad: puede venir como string (`polarity`) o como lista (`polarities`).
fn polarity_field(v: &serde_json::Value) -> Option<String> {
    match v.get("polarity").or_else(|| v.get("polarities"))? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => {
            let joined: Vec<String> = a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            if joined.is_empty() {
                None
            } else {
                Some(joined.join(", "))
            }
        }
        _ => None,
    }
}

/// Lista de polaridades (warframes): siempre como array JSON.
fn polarities_field(v: &serde_json::Value) -> Option<String> {
    let arr = match v.get("polarities") {
        Some(serde_json::Value::Array(a)) if !a.is_empty() => a,
        _ => return None,
    };
    let list: Vec<&str> = arr.iter().filter_map(|x| x.as_str()).collect();
    if list.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&list).unwrap_or_default())
    }
}

/// Fuentes de drop (`drops` de WFCD): array de
/// `{location, type, chance, rarity}`.
///
/// Se guarda tal cual (JSON) porque cada entrada tiene una forma distinta
/// según el juego: un mod tiene `location` + `chance`, una reliquia tiene
/// `type` + `probabilities`, etc.
fn drops_field(v: &serde_json::Value) -> Option<String> {
    let arr = match v.get("drops") {
        Some(serde_json::Value::Array(a)) if !a.is_empty() => a,
        _ => return None,
    };
    // Solo guardamos drops con alguna señal de dónde sale el item.
    let useful: Vec<&serde_json::Value> = arr
        .iter()
        .filter(|d| {
            d.get("location").is_some()
                || d.get("type").is_some()
                || d.get("chance").is_some()
                || d.get("itemName").is_some()
        })
        .collect();
    if useful.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&useful).unwrap_or_default())
    }
}

/// Stats de combate resumidas (warframes, armas, pets…).
///
/// Se guarda un subconjunto de campos numéricos interesting para responder
/// "qué tan fuerte es" sin cargar el JSON entero en el contexto del modelo.
const STAT_KEYS: &[&str] = &[
    // Warframes
    "health", "shield", "armor", "power", "energy", "stamina", "sprintSpeed",
    // Armas
    "totalDamage", "damagePerShot", "criticalChance", "criticalMultiplier",
    "fireRate", "magazineSize", "accuracy", "reloadTime", "procChance",
    "multishot", "projectileSpeed", "impactDamage", "slideDamage",
    // Cosméticos / general
    "buildTime", "masteryReq", "itemCount",
];

fn stats_field(v: &serde_json::Value) -> Option<String> {
    let mut map = serde_json::Map::new();
    for key in STAT_KEYS {
        match v.get(*key) {
            Some(serde_json::Value::Number(n)) => {
                map.insert((*key).to_string(), serde_json::Value::Number(n.clone()));
            }
            Some(serde_json::Value::String(s)) => {
                map.insert((*key).to_string(), serde_json::Value::String(s.clone()));
            }
            _ => {}
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map).to_string())
    }
}

/// Habilidades del warframe (array de `{name, description}`).
fn abilities_field(v: &serde_json::Value) -> Option<String> {
    let arr = match v.get("abilities") {
        Some(serde_json::Value::Array(a)) if !a.is_empty() => a,
        _ => return None,
    };
    let list: Vec<serde_json::Value> = arr
        .iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|x| x.as_str())?;
            let desc = a.get("description").and_then(|x| x.as_str()).unwrap_or("");
            Some(serde_json::json!({ "name": name, "description": desc }))
        })
        .collect();
    if list.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&list).unwrap_or_default())
    }
}

/// Rango de precio en platina (`marketCost` de WFCD).
fn market_cost_field(v: &serde_json::Value) -> Option<String> {
    let mc = v.get("marketCost")?;
    match mc {
        serde_json::Value::Object(_) => Some(mc.to_string()),
        serde_json::Value::Array(a) if !a.is_empty() => Some(mc.to_string()),
        _ => None,
    }
}

impl WfcdItem {
    /// Convierte un objeto JSON de WFCD en un item (o `None` si no es válido).
    fn from_value(v: &serde_json::Value) -> Option<Self> {
        let unique_name = v.get("uniqueName")?.as_str()?.to_string();
        let name = v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or(unique_name.as_str())
            .to_string();

        let components = v.get("components").and_then(|c| {
            let empty = c.as_array().is_some_and(|a| a.is_empty());
            if c.is_null() || empty {
                None
            } else {
                Some(c.to_string())
            }
        });

        Some(Self {
            unique_name,
            name,
            description: str_field(v, "description"),
            item_type: str_field(v, "type"),
            category: str_field(v, "category"),
            subcategory: str_field(v, "productCategory").or_else(|| str_field(v, "subcategory")),
            rarity: str_field(v, "rarity"),
            polarity: polarity_field(v),
            mastery_req: num_field(v, "masteryReq"),
            tradable: bool_field(v, "tradable"),
            image_name: str_field(v, "imageName"),
            wikia_url: str_field(v, "wikiaUrl"),
            build_price: num_field(v, "buildPrice"),
            build_quantity: num_field(v, "buildQuantity"),
            build_time: num_field(v, "buildTime"),
            components,
            drops: drops_field(v),
            polarities: polarities_field(v),
            stats: stats_field(v),
            abilities: abilities_field(v),
            passive: str_field(v, "passiveDescription"),
            market_cost: market_cost_field(v),
        })
    }
}

/// Sincroniza la BD local con el JSON de WFCD (release latest de GitHub).
#[allow(dead_code)]
pub fn sync_wfcd(conn: &Connection) -> Result<usize, String> {
    sync_wfcd_with_progress(conn, &mut |_| {})
}

/// Igual que [`sync_wfcd`], pero reporta el avance (0-100) por callback.
pub fn sync_wfcd_with_progress(
    conn: &Connection,
    on_progress: &mut dyn FnMut(u8),
) -> Result<usize, String> {
    let items = fetch_wfcd_items(on_progress)?;
    write_wfcd_items(conn, &items, on_progress)
}

/// Sincroniza usando la BD global.
///
/// La descarga del JSON se hace **sin** tomar el lock de la BD: mientras baja
/// (puede tardar varios segundos) las tools siguen respondiendo con los datos
/// que ya hay en SQLite.
pub fn sync_wfcd_global(on_progress: &mut dyn FnMut(u8)) -> Result<usize, String> {
    on_progress(5);
    let items = fetch_wfcd_items(on_progress)?;
    let arc = get_items_db().ok_or("BD de items no inicializada")?;
    let conn = arc.lock().map_err(|_| "BD de items bloqueada".to_string())?;
    write_wfcd_items(&conn, &items, on_progress)
}

/// Categorías que publica WFCD en `data/json/`.
const WFCD_CATEGORIES: &[&str] = &[
    "Arcanes",
    "Arch-Gun",
    "Arch-Melee",
    "Archwing",
    "Components",
    "Enemy",
    "Fish",
    "Gear",
    "Glyphs",
    "Honoria",
    "Melee",
    "Misc",
    "Mods",
    "Node",
    "Pets",
    "Primary",
    "Quests",
    "Railjack",
    "Relics",
    "Resources",
    "Secondary",
    "SentinelWeapons",
    "Sentinels",
    "Sigils",
    "Skins",
    "Warframes",
];

/// Base de los archivos por categoría del repo oficial `WFCD/warframe-items`.
pub const WFCD_RAW_BASE: &str =
    "https://raw.githubusercontent.com/WFCD/warframe-items/master/data/json";

/// JSON agregado (lo publicaba WFCD como asset de release).
const WFCD_AGGREGATE_URL: &str =
    "https://github.com/WFCD/warframe-items/releases/latest/download/items.json";

/// Descarga y parsea los datos de WFCD (no toca la BD).
///
/// Fuentes, en orden:
/// 1. El `items.json` agregado de la última release (si upstream lo publica).
/// 2. Los JSON por categoría (`data/json/*.json`), que es como está hoy el repo.
fn fetch_wfcd_items(on_progress: &mut dyn FnMut(u8)) -> Result<Vec<WfcdItem>, String> {
    let client = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(90)))
            .user_agent("cehalon-mate/1.0 (+https://github.com/FabianGallardo-coder/Cephalon_Mate)")
            .build(),
    );

    on_progress(8);
    if let Some(items) = fetch_aggregate(&client) {
        on_progress(50);
        return Ok(items);
    }

    let mut all: Vec<WfcdItem> = Vec::new();
    let total = WFCD_CATEGORIES.len();
    for (i, cat) in WFCD_CATEGORIES.iter().enumerate() {
        let url = format!("{WFCD_RAW_BASE}/{cat}.json");
        let resp = client
            .get(&url)
            .call()
            .map_err(|e| format!("GET {cat}: {e}"))?;
        if !resp.status().is_success() {
            continue;
        }
        let text = resp.into_body().read_to_string().map_err(|e| format!("{cat}: response text: {e}"))?;
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&text)
            .map_err(|e| format!("{cat}: parse JSON: {e}"))?;
        let items: Vec<WfcdItem> = parsed.iter().filter_map(WfcdItem::from_value).collect();
        all.extend(items);

        let pct = 10 + (70 * (i + 1) / total) as u8;
        on_progress(pct);
    }

    if all.is_empty() {
        return Err("no se pudo descargar ningún item de WFCD".into());
    }
    Ok(all)
}

/// Intenta el JSON agregado de la release (puede no existir).
fn fetch_aggregate(client: &ureq::Agent) -> Option<Vec<WfcdItem>> {
    let resp = client.get(WFCD_AGGREGATE_URL).call().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.into_body().read_to_string().ok()?;
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&text).ok()?;
    let items: Vec<WfcdItem> = parsed.iter().filter_map(WfcdItem::from_value).collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

/// Escribe (inserta/actualiza) los items descargados en la BD.
fn write_wfcd_items(
    conn: &Connection,
    items: &[WfcdItem],
    on_progress: &mut dyn FnMut(u8),
) -> Result<usize, String> {
    let tx = conn.unchecked_transaction().map_err(|e| format!("tx: {e}"))?;
    let mut inserted = 0;
    let mut updated = 0;

    for item in items {
        let exists: bool = tx
            .query_row(
                "SELECT 1 FROM items WHERE unique_name = ?1",
                params![&item.unique_name],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| format!("check exists: {e}"))?
            .is_some();

        let tradable = item.tradable.map(|b| if b { 1 } else { 0 });
        // En el schema actual de WFCD, "components" es la receta de crafteo:
        // va a la columna "ingredients". "components" (set) queda vacío.
        let ingredients_json = item.components.clone();
        let components_json: Option<String> = None;

        if exists {
            tx.execute(
                r#"
                UPDATE items SET
                    name = ?1, description = ?2, type = ?3, category = ?4, subcategory = ?5,
                    rarity = ?6, polarity = ?7, mastery_req = ?8, tradable = ?9,
                    image_name = ?10, build_price = ?11, build_quantity = ?12,
                    build_time = ?13, ingredients = ?14, components = ?15,
                    wikia_url = ?16, drops = ?17, polarities = ?18, stats = ?19,
                    abilities = ?20, passive = ?21, market_cost = ?22,
                    updated_at = CURRENT_TIMESTAMP
                WHERE unique_name = ?23
                "#,
                params![
                    &item.name,
                    &item.description,
                    &item.item_type,
                    &item.category,
                    &item.subcategory,
                    &item.rarity,
                    &item.polarity,
                    &item.mastery_req,
                    &tradable,
                    &item.image_name,
                    &item.build_price,
                    &item.build_quantity,
                    &item.build_time,
                    &ingredients_json,
                    &components_json,
                    &item.wikia_url,
                    &item.drops,
                    &item.polarities,
                    &item.stats,
                    &item.abilities,
                    &item.passive,
                    &item.market_cost,
                    &item.unique_name,
                ],
            )
            .map_err(|e| format!("update: {e}"))?;
            updated += 1;
        } else {
            tx.execute(
                r#"
                INSERT INTO items (
                    unique_name, name, description, type, category, subcategory,
                    rarity, polarity, mastery_req, tradable, image_name,
                    build_price, build_quantity, build_time, ingredients, components,
                    wikia_url, drops, polarities, stats, abilities, passive, market_cost
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                          ?18, ?19, ?20, ?21, ?22, ?23)
                "#,
                params![
                    &item.unique_name,
                    &item.name,
                    &item.description,
                    &item.item_type,
                    &item.category,
                    &item.subcategory,
                    &item.rarity,
                    &item.polarity,
                    &item.mastery_req,
                    &tradable,
                    &item.image_name,
                    &item.build_price,
                    &item.build_quantity,
                    &item.build_time,
                    &ingredients_json,
                    &components_json,
                    &item.wikia_url,
                    &item.drops,
                    &item.polarities,
                    &item.stats,
                    &item.abilities,
                    &item.passive,
                    &item.market_cost,
                ],
            )
            .map_err(|e| format!("insert: {e}"))?;
            inserted += 1;
        }
    }

    on_progress(95);
    tx.commit().map_err(|e| format!("commit: {e}"))?;
    on_progress(100);
    Ok(inserted + updated)
}

/// Formatea un item para la respuesta del modelo (tool `buscar_item`).
pub fn format_item_for_llm(item: &ItemRow) -> String {
    let mut out = String::new();
    out.push_str(&format!("**{}**", item.name));
    if item.unique_name != item.name {
        out.push_str(&format!(" (`{}`)", item.unique_name));
    }
    out.push('\n');

    if let Some(ref d) = item.description {
        if !d.is_empty() {
            out.push_str(&format!("> {d}\n\n"));
        }
    }

    let mut meta = Vec::new();
    if let Some(ref t) = item.item_type {
        meta.push(format!("Tipo: {t}"));
    }
    if let Some(ref c) = item.category {
        meta.push(format!("Categoría: {c}"));
    }
    if let Some(ref s) = item.subcategory {
        meta.push(format!("Subcategoría: {s}"));
    }
    if let Some(ref r) = item.rarity {
        meta.push(format!("Rareza: {r}"));
    }
    if let Some(m) = item.mastery_req {
        meta.push(format!("Maestría: {m}"));
    }
    if let Some(t) = item.tradable {
        meta.push(if t == 1 { "Tradable: Sí" } else { "Tradable: No" }.into());
    }
    if let Some(ref p) = item.polarity {
        if !p.is_empty() {
            meta.push(format!("Polaridad: {p}"));
        }
    }
    if let Some(bp) = item.build_price {
        if bp > 0 {
            meta.push(format!("Precio de construcción: {bp} créditos"));
        }
    }
    if let Some(bq) = item.build_quantity {
        if bq > 0 {
            meta.push(format!("Cantidad por build: {bq}"));
        }
    }
    if let Some(bt) = item.build_time {
        if bt > 0 {
            let hours = bt / 3600;
            let mins = (bt % 3600) / 60;
            meta.push(format!("Tiempo de build: {}h {}m", hours, mins));
        }
    }
    if !meta.is_empty() {
        out.push_str(&meta.join(" · "));
        out.push('\n');
    }

    if let Some(ref ing) = item.ingredients {
        if !ing.is_empty() && ing != "[]" {
            out.push_str(&format!("\n**Ingredientes:** {ing}"));
        }
    }
    if let Some(ref comp) = item.components {
        if !comp.is_empty() && comp != "[]" {
            out.push_str(&format!("\n**Componentes (set):** {comp}"));
        }
    }

    // Stats de combate (warframes, armas).
    if let Some(stats) = format_stats_for_llm(item) {
        out.push_str(&format!("\n**Stats:** {stats}"));
    }

    // Habilidades y pasiva (warframes).
    if let Some(ref abilities) = item.abilities {
        if !abilities.is_empty() && abilities != "[]" {
            out.push_str(&format!("\n**Habilidades:** {abilities}"));
        }
    }
    if let Some(ref p) = item.passive {
        if !p.is_empty() {
            out.push_str(&format!("\n**Pasiva:** {p}"));
        }
    }

    // Dónde encontrarlo.
    let is_warframe = item
        .item_type
        .as_deref()
        .map(|t| t.to_lowercase().contains("warframe"))
        .unwrap_or(false);
    let has_drops = item
        .drops
        .as_deref()
        .map(|d| !parse_drops(d).is_empty())
        .unwrap_or(false);
    if has_drops || !is_warframe {
        let where_to = format_where_to_find(item);
        if !where_to.is_empty() {
            out.push_str(&where_to);
        }
    } else {
        out.push_str(
            "\n**Dónde encontrarlo:** no hay drops de enemigo; se consigue por crafteo \
             (receta abajo), en la tienda del Duviri o en recompensas de evento.\n",
        );
    }

    out
}

/// Una fuente de drop parseada.
#[derive(Debug, Clone)]
pub struct DropSource {
    /// Nombre del enemigo / lugar / misión.
    pub location: String,
    /// Tipo de drop (a veces difiere del nombre, p. ej. "Spectral Serration").
    pub kind: Option<String>,
    /// Probabilidad en **porcentaje** (0-100), ya normalizada.
    pub chance: Option<f64>,
    /// Rareza del drop.
    pub rarity: Option<String>,
}

/// Normaliza la probabilidad de WFCD a porcentaje.
///
/// Upstream usa dos formatos en el mismo campo `chance`:
/// - enemigos: fracción (`0.22` = 22%),
/// - reliquias/conjunctions: porcentaje ya (`8.6` = 8.6%).
///
/// La regla: si la location parece de reliquia (tiene `/`, "Rotation" o
/// "Conjunction") el valor ya es porcentaje; si no, es fracción salvo que
/// supere 1.
fn chance_to_percent(chance: f64, location: &str) -> f64 {
    let looks_like_relic = location.contains('/')
        || location.contains("Rotation")
        || location.contains("Conjunction")
        || location.contains("Relic");
    if looks_like_relic || chance > 1.0 {
        chance
    } else {
        chance * 100.0
    }
}

/// Formatea un porcentaje sin decimales inútiles (22% en vez de 22.00%).
fn fmt_pct(v: f64) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{v:.0}%")
    } else {
        format!("{v:.2}%")
    }
}

/// Parsea el JSON de `drops` de un item.
///
/// Tolera las variantes de WFCD: a veces el lugar es `location`, a veces
/// `itemName`/`name`; la probabilidad puede ser `chance` o `probability`.
pub fn parse_drops(json: &str) -> Vec<DropSource> {
    let arr: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match arr.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|d| {
            let location = d
                .get("location")
                .or_else(|| d.get("itemName"))
                .or_else(|| d.get("name"))
                .and_then(|x| x.as_str())
                .map(str::to_string)?;
            let kind = d
                .get("type")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let chance = d
                .get("chance")
                .or_else(|| d.get("probability"))
                .and_then(|x| x.as_f64())
                .map(|c| chance_to_percent(c, &location));
            let rarity = d
                .get("rarity")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            Some(DropSource {
                location,
                kind,
                chance,
                rarity,
            })
        })
        .collect()
}

/// Formatea la sección "dónde encontrarlo" de un item.
///
/// Devuelve cadena vacía si el item no tiene datos de drop (p. ej. un mod
/// seasonal o un item que solo se compra).
pub fn format_where_to_find(item: &ItemRow) -> String {
    let mut out = String::new();

    // Polaridades (warframes): con qué polarity slot encaja el mod.
    if let Some(ref pol) = item.polarities {
        let list = parse_string_list(pol);
        if !list.is_empty() {
            out.push_str(&format!(
                "\n**Polaridades:** {}",
                list.iter()
                    .map(|p| polarity_display(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    let drops = item.drops.as_ref().map(|d| parse_drops(d)).unwrap_or_default();
    if drops.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("\n**Dónde encontrarlo:** sin datos de drop en WFCD (item de tienda, evento o recompensa).");
        return out;
    }

    out.push_str("\n**Dónde encontrarlo:**");
    // Top 10 por probabilidad (las más útiles para el Tenno).
    let mut sorted = drops.clone();
    sorted.sort_by(|a, b| {
        b.chance
            .unwrap_or(0.0)
            .partial_cmp(&a.chance.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for d in sorted.iter().take(10) {
        let mut line = format!("- {}", d.location);
        if let Some(ref k) = d.kind {
            if k != &d.location {
                line.push_str(&format!(" ({k})"));
            }
        }
        if let Some(c) = d.chance {
            line.push_str(&format!(" — {}", fmt_pct(c)));
        }
        if let Some(ref r) = d.rarity {
            line.push_str(&format!(" · {r}"));
        }
        out.push_str(&format!("\n{line}"));
    }
    if drops.len() > 10 {
        out.push_str(&format!("\n… y {} fuentes más", drops.len() - 10));
    }
    out.push('\n');
    out
}

/// Polaridad en nombre legible (madurai, vazarin, naramon…).
fn polarity_display(p: &str) -> String {
    match p.to_lowercase().as_str() {
        "madurai" => "Madurai".into(),
        "vazarin" => "Vazarin".into(),
        "naramon" => "Naramon".into(),
        "umbra" => "Umbra".into(),
        "zenurik" => "Zenurik".into(),
        other => other.to_string(),
    }
}

/// Convierte un JSON array de strings en `Vec<String>`.
fn parse_string_list(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

/// Lista los mods compatibles con las polaridades de un warframe.
///
/// Un mod encaja si su campo `polarity` coincide con alguna de las
/// polaridades del warframe (`polarities`). Es el mismo criterio que usa el
/// juego al instalar un mod en un slot.
pub fn mods_for_polarities(conn: &Connection, polarities: &[String], limit: usize) -> Result<Vec<ItemRow>, String> {
    if polarities.is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!(
        r#"
        SELECT {ITEM_COLUMNS}
        FROM items
        WHERE lower(polarity) IN ({})
          AND (type LIKE '%Mod%' OR category = 'Mods')
          AND tradable = 1
        ORDER BY
            CASE rarity
                WHEN 'Exotic' THEN 0 WHEN 'Prime' THEN 1 WHEN 'Rare' THEN 2
                WHEN 'Uncommon' THEN 3 WHEN 'Common' THEN 4 ELSE 5 END,
            name
        LIMIT ?{}
        "#,
        std::iter::repeat_n("?", polarities.len()).collect::<Vec<_>>().join(", "),
        polarities.len() + 1
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let limit_i = limit as i64;
    let lowered: Vec<String> = polarities.iter().map(|p| p.to_lowercase()).collect();
    let mut params: Vec<&dyn rusqlite::ToSql> =
        lowered.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    params.push(&limit_i as &dyn rusqlite::ToSql);

    let rows = stmt
        .query_map(params.as_slice(), row_to_item)
        .map_err(|e| format!("query: {e}"))?;

    let mut out: Vec<ItemRow> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for r in rows {
        let item = r.map_err(|e| format!("row: {e}"))?;
        // Varios mods comparten nombre pero son de slot distinto (p. ej. el
        // "Aviator" de rifle, escopeta y pistola): para el Tenno es uno solo.
        let key = item.name.to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        out.push(item);
    }
    Ok(out)
}

/// Polaridades de un item (warframe) parseadas desde la columna `polarities`.
pub fn polarities_of(item: &ItemRow) -> Vec<String> {
    item.polarities
        .as_ref()
        .map(|p| parse_string_list(p))
        .unwrap_or_default()
}

/// Resuelve los nombres legibles de los ingredientes de una receta.
///
/// WFCD guarda los componentes como `uniqueName`; para el Tenno sirve mucho
/// más "Neurode ×1" que "/Lotus/Types/Items/MiscItems/Neurode".
pub fn resolve_ingredients(conn: &Connection, ingredients_json: &str) -> Option<String> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(ingredients_json).ok()?;
    if arr.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    for comp in &arr {
        let un = comp.get("uniqueName").and_then(|x| x.as_str());
        let qty = comp.get("itemCount").and_then(|x| x.as_i64()).unwrap_or(1);
        let name = un
            .and_then(|u| name_by_unique_name(conn, u))
            .or_else(|| {
                // El uniqueName a veces ya trae el nombre legible al final.
                un.and_then(|u| u.rsplit('/').next().map(str::to_string))
            })
            .unwrap_or_else(|| un.unwrap_or("?").to_string());
        parts.push(format!("{name} ×{qty}"));
    }
    Some(parts.join(", "))
}

/// Nombre legible de un item a partir de su `unique_name`.
fn name_by_unique_name(conn: &Connection, unique_name: &str) -> Option<String> {
    conn.query_row(
        "SELECT name FROM items WHERE unique_name = ?1",
        params![unique_name],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

/// Formatea las stats de combate de un item (JSON plano → texto legible).
pub fn format_stats_for_llm(item: &ItemRow) -> Option<String> {
    let stats = item.stats.as_ref()?;
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(stats).ok()?;
    if map.is_empty() {
        return None;
    }

    const LABELS: &[(&str, &str)] = &[
        ("health", "Vida"),
        ("shield", "Escudo"),
        ("armor", "Armadura"),
        ("power", "Poder"),
        ("energy", "Energía"),
        ("stamina", "Estamina"),
        ("sprintSpeed", "Velocidad sprint"),
        ("totalDamage", "Daño total"),
        ("damagePerShot", "Daño por disparo"),
        ("criticalChance", "Crítico"),
        ("criticalMultiplier", "Multiplicador crítico"),
        ("fireRate", "Cadencia"),
        ("magazineSize", "Cargador"),
        ("accuracy", "Precisión"),
        ("reloadTime", "Recarga"),
        ("procChance", "Prob. de estado"),
        ("multishot", "Multidisparo"),
        ("projectileSpeed", "Vel. proyectil"),
        ("masteryReq", "Maestría"),
    ];

    let mut parts = Vec::new();
    for (key, label) in LABELS {
        if let Some(v) = map.get(*key) {
            let text = match v {
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        i.to_string()
                    } else {
                        format!("{:.2}", n.as_f64().unwrap_or(0.0))
                    }
                }
                serde_json::Value::String(s) => s.clone(),
                _ => continue,
            };
            parts.push(format!("{label}: {text}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

/// Formatea las habilidades de un warframe ("Rhino Charge: …; Iron Skin: …").
pub fn format_abilities_for_llm(item: &ItemRow) -> Option<String> {
    let raw = item.abilities.as_ref()?;
    let list: Vec<serde_json::Value> = serde_json::from_str(raw).ok()?;
    if list.is_empty() {
        return None;
    }
    let parts: Vec<String> = list
        .iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|x| x.as_str())?;
            let desc = a.get("description").and_then(|x| x.as_str()).unwrap_or("");
            if desc.is_empty() {
                Some(name.to_string())
            } else {
                Some(format!("{name}: {desc}"))
            }
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// Precio cacheado de un item (warframe.market).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceCache {
    pub item_slug: String,
    pub item_name: String,
    pub sell_min: Option<i64>,
    pub sell_max: Option<i64>,
    pub sell_avg: Option<i64>,
    pub buy_max: Option<i64>,
    pub buy_min: Option<i64>,
    pub buy_avg: Option<i64>,
    pub volume_24h: Option<i64>,
    pub updated_at: String,
    pub expires_at: String,
}

/// Estructura de respuesta de `/v2/orders/item/{slug}`.
#[derive(Debug, Deserialize)]
struct OrdersResponse {
    #[serde(rename = "include")]
    include: OrdersInclude,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrdersInclude {
    #[serde(rename = "item")]
    item: OrdersItem,
    #[serde(rename = "orders")]
    orders: Vec<Order>,
}

#[derive(Debug, Deserialize)]
struct OrdersItem {
    #[allow(dead_code)]
    #[serde(rename = "id")]
    id: String,
    #[allow(dead_code)]
    #[serde(rename = "slug")]
    slug: String,
    #[serde(rename = "i18n")]
    i18n: OrdersI18n,
}

#[derive(Debug, Deserialize)]
struct OrdersI18n {
    #[serde(rename = "en")]
    en: OrdersName,
}

#[derive(Debug, Deserialize)]
struct OrdersName {
    #[serde(rename = "name")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct Order {
    #[serde(rename = "type")]
    order_type: String,      // "sell" | "buy"
    #[serde(rename = "platinum")]
    platinum: i64,
    #[serde(rename = "quantity")]
    quantity: i64,
    #[serde(rename = "visible")]
    visible: bool,
    #[serde(rename = "user")]
    user: OrderUser,
}

#[derive(Debug, Deserialize)]
struct OrderUser {
    #[serde(rename = "status")]
    status: String,          // "online" | "ingame" | "offline"
    #[allow(dead_code)]
    #[serde(rename = "reputation")]
    reputation: i64,
}

/// TTL del caché de precios (10 minutos).
const PRICE_TTL_SECS: i64 = 600;

/// Lee el precio del caché si no ha expirado. No hace fetch.
pub fn get_price_cached(conn: &Connection, item_slug: &str) -> Option<PriceCache> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.query_row(
        r#"
        SELECT item_slug, item_name, sell_min, sell_max, sell_avg,
               buy_max, buy_min, buy_avg, volume_24h, updated_at, expires_at
        FROM prices
        WHERE item_slug = ?1 AND expires_at > ?2
        "#,
        params![item_slug, &now],
        |row| {
            Ok(PriceCache {
                item_slug: row.get(0)?,
                item_name: row.get(1)?,
                sell_min: row.get(2)?,
                sell_max: row.get(3)?,
                sell_avg: row.get(4)?,
                buy_max: row.get(5)?,
                buy_min: row.get(6)?,
                buy_avg: row.get(7)?,
                volume_24h: row.get(8)?,
                updated_at: row.get(9)?,
                expires_at: row.get(10)?,
            })
        },
    )
    .optional()
    .ok()
    .flatten()
}

/// Obtiene el precio: primero caché, si no hay o expiró → fetch a warframe.market + upsert.
#[allow(dead_code)]
pub fn get_price(conn: &Connection, item_slug: &str) -> Result<PriceCache, String> {
    if let Some(c) = get_price_cached(conn, item_slug) {
        return Ok(c);
    }
    fetch_and_cache_price(item_slug)
}

/// Hace el fetch HTTP a warframe.market y hace upsert en el caché.
/// Se llama SIN mantener el lock de la BD (el lock se toma solo para el upsert).
pub fn fetch_and_cache_price(item_slug: &str) -> Result<PriceCache, String> {
    let url = format!("https://api.warframe.market/v2/orders/item/{}", urlencoding::encode(item_slug));
    let client = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .build(),
    );

    let resp = client
        .get(&url)
        .header("accept", "application/json")
        .call()
        .map_err(|e| format!("HTTP GET: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), url));
    }

    let data: OrdersResponse = resp
        .into_body()
        .read_json()
        .map_err(|e| format!("parse JSON: {e}"))?;

    if let Some(err) = data.error {
        return Err(format!("API error: {err}"));
    }

    let item_name = data.include.item.i18n.en.name.clone();

    // Filtrar órdenes visibles de usuarios online/ingame (no offline).
    let visible_orders: Vec<&Order> = data
        .include
        .orders
        .iter()
        .filter(|o| o.visible && (o.user.status == "online" || o.user.status == "ingame"))
        .collect();

    if visible_orders.is_empty() {
        return Err("no hay órdenes visibles de usuarios activos".into());
    }

    let sells: Vec<i64> = visible_orders
        .iter()
        .filter(|o| o.order_type == "sell")
        .map(|o| o.platinum)
        .collect();
    let buys: Vec<i64> = visible_orders
        .iter()
        .filter(|o| o.order_type == "buy")
        .map(|o| o.platinum)
        .collect();

    let sell_min = sells.iter().min().copied();
    let sell_max = sells.iter().max().copied();
    let sell_avg = if !sells.is_empty() {
        Some(sells.iter().sum::<i64>() / sells.len() as i64)
    } else {
        None
    };
    let buy_max = buys.iter().max().copied();
    let buy_min = buys.iter().min().copied();
    let buy_avg = if !buys.is_empty() {
        Some(buys.iter().sum::<i64>() / buys.len() as i64)
    } else {
        None
    };

    let volume_24h = visible_orders.iter().map(|o| o.quantity).sum::<i64>();

    let now = chrono::Utc::now().to_rfc3339();
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::minutes(5))
        .unwrap()
        .to_rfc3339();

    let price = PriceCache {
        item_slug: item_slug.to_string(),
        item_name,
        sell_min,
        sell_max,
        sell_avg,
        buy_max,
        buy_min,
        buy_avg,
        volume_24h: Some(volume_24h),
        updated_at: now.clone(),
        expires_at,
    };

    // Upsert en BD (aquí sí tomamos el lock brevemente).
    let db = get_items_db()
        .ok_or("BD de items no inicializada")?;
    let conn = db.lock().unwrap_or_else(|e| e.into_inner());
    upsert_price(&conn, &price)?;

    Ok(price)
}

fn upsert_price(conn: &Connection, price: &PriceCache) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO prices (
            item_slug, item_name, sell_min, sell_max, sell_avg,
            buy_max, buy_min, buy_avg, volume_24h, updated_at, expires_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(item_slug) DO UPDATE SET
            item_name = excluded.item_name,
            sell_min = excluded.sell_min,
            sell_max = excluded.sell_max,
            sell_avg = excluded.sell_avg,
            buy_max = excluded.buy_max,
            buy_min = excluded.buy_min,
            buy_avg = excluded.buy_avg,
            volume_24h = excluded.volume_24h,
            updated_at = excluded.updated_at,
            expires_at = excluded.expires_at
        "#,
        params![
            price.item_slug,
            price.item_name,
            price.sell_min,
            price.sell_max,
            price.sell_avg,
            price.buy_max,
            price.buy_min,
            price.buy_avg,
            price.volume_24h,
            price.updated_at,
            price.expires_at
        ],
    )
    .map_err(|e| format!("upsert price: {e}"))?;
    Ok(())
}


/// Formatea un PriceCache para la respuesta del modelo (tool `consultar_precio`).
pub fn format_price_for_llm(price: &PriceCache) -> String {
    let mut out = String::new();
    out.push_str(&format!("**{}** ({})\n", price.item_name, price.item_slug));
    out.push_str(&format!("🕐 Actualizado: {} (expira en ~10 min)\n", price.updated_at));

    if let Some(v) = price.sell_min {
        out.push_str(&format!("💰 **Venta** (sell orders online):\n"));
        out.push_str(&format!("  • Mínimo: {}p", v));
        if let Some(max) = price.sell_max {
            out.push_str(&format!(" • Máximo: {}p", max));
        }
        if let Some(avg) = price.sell_avg {
            out.push_str(&format!(" • Media: {}p", avg));
        }
        out.push('\n');
    }

    if let Some(v) = price.buy_max {
        out.push_str(&format!("💵 **Compra** (buy orders online):\n"));
        out.push_str(&format!("  • Máximo: {}p", v));
        if let Some(min) = price.buy_min {
            out.push_str(&format!(" • Mínimo: {}p", min));
        }
        if let Some(avg) = price.buy_avg {
            out.push_str(&format!(" • Media: {}p", avg));
        }
        out.push('\n');
    }

    if let Some(vol) = price.volume_24h {
        out.push_str(&format!("📊 Volumen 24h (órdenes activas): {vol}\n"));
    }

    out.push_str(&format!("(fuente: warframe.market, caché {}s)", PRICE_TTL_SECS));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BD en memoria con el esquema completo (para tests sin tocar `data/`).
    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("mem db");
        create_schema(&conn).expect("schema");
        conn
    }

    /// Inserta un item de prueba.
    fn insert_test_item(conn: &Connection, unique_name: &str, name: &str, sql_extra: &str) {
        conn.execute(
            &format!(
                r#"
                INSERT INTO items (
                    unique_name, name, type, category, rarity, polarity,
                    tradable, drops, polarities, stats, abilities, passive
                ) VALUES (?1, ?2, 'Primary Mod', 'Mods', 'Rare', ?3, 1, ?4, ?5, ?6, ?7, ?8)
                {sql_extra}
                "#
            ),
            params![
                unique_name,
                name,
                "madurai",
                r#"[{"location":"Charger","type":"Serration","chance":0.22,"rarity":"Rare"}]"#,
                r#"["vazarin"]"#,
                r#"{"totalDamage":100}"#,
                "[]",
                "Pasiva de prueba",
            ],
        )
        .expect("insert test item");
    }

    #[test]
    fn db_init_creates_tables() {
        let conn = init_db().expect("init_db");
        let count = count_items(&conn).expect("count");
        println!("items en BD vacía: {count}");
        // Verificar que la tabla prices existe
        let has_prices: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='prices'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(has_prices, 1, "tabla prices debe existir");
    }

    #[test]
    fn schema_has_new_columns() {
        let conn = mem_db();
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('items')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for c in [
            "drops",
            "polarities",
            "stats",
            "abilities",
            "passive",
            "market_cost",
            "wikia_url",
        ] {
            assert!(cols.contains(&c.to_string()), "falta la columna {c}");
        }
    }

    #[test]
    fn migration_adds_missing_columns() {
        // BD "vieja": items sin las columnas nuevas.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE items (unique_name TEXT PRIMARY KEY, name TEXT NOT NULL);",
        )
        .unwrap();
        migrate_schema(&conn).expect("migración");

        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('items')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(cols.contains(&"drops".to_string()));
        assert!(cols.contains(&"abilities".to_string()));
    }

    #[test]
    fn get_item_by_name_returns_correct_polarity() {
        // Regresión: la columna `polarity` se leía con el índice de `rarity`
        // (bug: devolvía la rareza como polaridad).
        let conn = mem_db();
        conn.execute(
            "INSERT INTO items (unique_name, name, rarity, polarity) VALUES ('/x', 'Serration', 'Rare', 'madurai')",
            [],
        )
        .unwrap();
        let item = get_item_by_name(&conn, "Serration").unwrap().unwrap();
        assert_eq!(item.polarity.as_deref(), Some("madurai"));
        assert_eq!(item.rarity.as_deref(), Some("Rare"));
    }

    #[test]
    fn parse_drops_reads_location_and_chance() {
        let drops = parse_drops(
            r#"[{"location":"Scorpion","type":"Serration","chance":0.22,"rarity":"Rare"},
                {"location":"Kuva Scorpion","type":"Serration","chance":0.22}]"#,
        );
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0].location, "Scorpion");
        // 0.22 en WFCD (enemigos) = 22%.
        assert_eq!(drops[0].chance, Some(22.0));
        assert_eq!(drops[0].rarity.as_deref(), Some("Rare"));
    }

    #[test]
    fn parse_drops_normalizes_relic_percentage() {
        // En reliquias WFCD ya pone el porcentaje (8.6 = 8.6%).
        let drops = parse_drops(
            r#"[{"location":"Ceres/Hapke (Spy), Rotation C","type":"Serration","chance":8.6}]"#,
        );
        assert_eq!(drops[0].chance, Some(8.6));
        assert!(
            drops[0].chance.unwrap() <= 100.0,
            "una probabilidad no puede pasar de 100%"
        );
    }

    #[test]
    fn parse_drops_never_exceeds_100_percent() {
        // Regresión: se mostraba "860.0%" por no normalizar la fracción.
        let drops = parse_drops(
            r#"[{"location":"Ceres/Hapke (Spy), Rotation C","chance":8.6},
                {"location":"Scorpion","chance":0.22}]"#,
        );
        for d in &drops {
            let c = d.chance.unwrap();
            assert!(c <= 100.0, "{}: chance {c} > 100%", d.location);
        }
    }

    #[test]
    fn parse_drops_tolerates_garbage() {
        assert!(parse_drops("no json").is_empty());
        assert!(parse_drops("[]").is_empty());
        assert!(parse_drops(r#"[{"sin":"location"}]"#).is_empty());
        // `itemName` es la alternativa válida.
        let d = parse_drops(r#"[{"itemName":"Crown","chance":0.5}]"#);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].location, "Crown");
    }

    #[test]
    fn drops_field_keeps_only_useful_entries() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"uniqueName":"/x","name":"Serration","polarity":"madurai",
                "drops":[{"location":"Charger","chance":0.2},{"nada":1}]}"#,
        )
        .unwrap();
        let item = WfcdItem::from_value(&v).unwrap();
        let drops = parse_drops(item.drops.as_deref().unwrap_or("[]"));
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].location, "Charger");
    }

    #[test]
    fn warframe_item_parses_stats_abilities_and_polarities() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"uniqueName":"/Lotus/Powersuits/Rhino/Rhino","name":"Rhino",
                "type":"Warframe","polarities":["vazarin","vazarin"],
                "health":270,"shield":455,"armor":240,"power":100,
                "passiveDescription":"Emit a shockwave",
                "abilities":[{"name":"Rhino Charge","description":"charges"},{"name":"Iron Skin","description":"armor"}]}"#,
        )
        .unwrap();
        let item = WfcdItem::from_value(&v).unwrap();
        assert_eq!(item.polarity.as_deref(), Some("vazarin, vazarin"));
        assert_eq!(item.passive.as_deref(), Some("Emit a shockwave"));

        let pols = item.polarities.as_deref().unwrap();
        assert!(pols.contains("vazarin"));
        assert!(!pols.contains("madurai"));

        let stats = item.stats.as_deref().unwrap();
        assert!(stats.contains("\"health\":270"));
        assert!(stats.contains("\"armor\":240"));

        let ab = item.abilities.as_deref().unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(ab).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["name"], "Rhino Charge");
    }

    #[test]
    fn where_to_find_lists_drop_sources() {
        let conn = mem_db();
        insert_test_item(&conn, "/Lotus/Upgrades/Mods/Rifle/Serration", "Serration", "");
        let item = get_item_by_name(&conn, "Serration").unwrap().unwrap();
        let text = format_where_to_find(&item);
        assert!(text.contains("Dónde encontrarlo"), "{text}");
        assert!(text.contains("Charger"), "{text}");
        assert!(text.contains("22%"), "{text}");
    }

    #[test]
    fn where_to_find_says_when_no_data() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO items (unique_name, name, type) VALUES ('/x', 'Mod X', 'Primary Mod')",
            [],
        )
        .unwrap();
        let item = get_item_by_name(&conn, "Mod X").unwrap().unwrap();
        let text = format_where_to_find(&item);
        assert!(text.contains("sin datos de drop"), "{text}");
    }

    #[test]
    fn mods_for_polarities_dedupes_by_name() {
        let conn = mem_db();
        // Tres "Aviator" distintos (rifle, escopeta, pistola) → 1 solo.
        for (un, comp) in [("/a", "Aviator"), ("/b", "Aviator"), ("/c", "Aviator")] {
            conn.execute(
                "INSERT INTO items (unique_name, name, type, category, polarity, tradable) VALUES (?1, 'Aviator', 'Primary Mod', 'Mods', 'vazarin', 1)",
                params![un],
            )
            .unwrap();
            let _ = comp;
        }
        conn.execute(
            "INSERT INTO items (unique_name, name, type, category, polarity, tradable) VALUES ('/d', 'Serration', 'Primary Mod', 'Mods', 'madurai', 1)",
            [],
        )
        .unwrap();

        let mods = mods_for_polarities(&conn, &["vazarin".to_string()], 10).unwrap();
        assert_eq!(mods.len(), 1, "debe deduplicar por nombre: {mods:?}");
        assert_eq!(mods[0].name, "Aviator");

        // Polaridad distinta → no aparece.
        let mods = mods_for_polarities(&conn, &["madurai".to_string()], 10).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "Serration");
    }

    #[test]
    fn mods_for_polarities_empty_list() {
        let conn = mem_db();
        assert!(mods_for_polarities(&conn, &[], 10).unwrap().is_empty());
    }

    #[test]
    fn item_formatter_includes_stats_and_drops() {
        let conn = mem_db();
        insert_test_item(&conn, "/Lotus/Upgrades/Mods/Rifle/Serration", "Serration", "");
        let item = get_item_by_name(&conn, "Serration").unwrap().unwrap();
        let text = format_item_for_llm(&item);
        assert!(text.contains("Serration"));
        assert!(text.contains("Dónde encontrarlo"));
        assert!(text.contains("Charger"));
        assert!(text.contains("Stats:"), "{text}");
    }

    #[test]
    #[ignore = "requiere red"]
    fn sync_wfcd_works() {
        let conn = init_db().expect("init_db");
        let n = sync_wfcd(&conn).expect("sync");
        assert!(n > 0, "debe insertar items");
        let count = count_items(&conn).expect("count");
        assert!(count > 0);
    }

    #[test]
    #[ignore = "requiere BD poblada"]
    fn search_items_works() {
        let conn = init_db().expect("init_db");
        let results = search_items(&conn, "soma", 5).expect("search");
        for r in &results {
            println!("{}", format_item_for_llm(r));
        }
        assert!(!results.is_empty());
    }

    /// QA de red: warframes y mods reales contra la BD sincronizada.
    #[test]
    #[ignore = "requiere red + BD poblada"]
    fn qa_real_warframes_and_mods() {
        let conn = init_db().expect("init_db");
        if count_items(&conn).unwrap_or(0) == 0 {
            sync_wfcd(&conn).expect("sync");
        }

        // Warframes: info completa y polaridades coherentes.
        for (name, expected) in [
            ("Rhino", "vazarin"),
            ("Nova", "madurai"),
            ("Mesa", "naramon"),
        ] {
            let item = get_item_by_name(&conn, name)
                .unwrap()
                .unwrap_or_else(|| panic!("{name} no está en la BD"));
            let pols = polarities_of(&item);
            assert!(
                pols.iter().any(|p| p.eq_ignore_ascii_case(expected)),
                "{name}: polaridades {:?} no contienen {expected}",
                pols
            );
            assert!(
                item.abilities.as_deref().unwrap_or("[]").len() > 50,
                "{name}: sin habilidades"
            );
            assert!(
                item.stats.as_deref().unwrap_or("").contains("\"health\""),
                "{name}: sin stats"
            );
        }

        // Mods: polaridad + fuentes de drop.
        for (name, pol) in [("Serration", "madurai"), ("Arrow Mutation", "naramon")] {
            let item = get_item_by_name(&conn, name)
                .unwrap()
                .unwrap_or_else(|| panic!("{name} no está en la BD"));
            assert_eq!(
                item.polarity.as_deref().unwrap_or("").to_lowercase(),
                pol,
                "{name}: polaridad inesperada"
            );
            let drops = parse_drops(item.drops.as_deref().unwrap_or("[]"));
            assert!(!drops.is_empty(), "{name}: sin fuentes de drop");
            assert!(
                drops.iter().any(|d| !d.location.is_empty()),
                "{name}: drop sin location"
            );
        }

        // Mods compatibles: Rhino (vazarin) → Serration (madurai) NO,
//        pero un mod vazarin sí.
        let rhino = get_item_by_name(&conn, "Rhino").unwrap().unwrap();
        let mods = mods_for_polarities(&conn, &polarities_of(&rhino), 50).unwrap();
        assert!(!mods.is_empty(), "Rhino debería tener mods compatibles");
        assert!(
            mods.iter().all(|m| {
                let p = m.polarity.clone().unwrap_or_default().to_lowercase();
                p == "vazarin" || p == "madurai" || p == "naramon"
            }),
            "mod con polaridad inválida"
        );
    }
}

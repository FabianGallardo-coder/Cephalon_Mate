//! Base de datos de items de Warframe (WFCD) + tool `buscar_item`.
//!
//! P09 del `PLAN_EJECUCION.md`: SQLite con esquema `items`, sync del JSON
//! de `warframe-items` (release latest), y tool real `buscar_item`.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

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

    Ok(conn)
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
}

/// Busca items por nombre (LIKE case-insensitive) — tool `buscar_item`.
pub fn search_items(conn: &Connection, query: &str, limit: usize) -> Result<Vec<ItemRow>, String> {
    let pattern = format!("%{}%", query.to_lowercase());
    let mut stmt = conn
        .prepare(
            r#"
            SELECT unique_name, name, description, type, category, subcategory,
                   rarity, polarity, mastery_req, tradable, image_name,
                   build_price, build_quantity, build_time, ingredients, components
            FROM items
            WHERE lower(name) LIKE ?1
               OR lower(unique_name) LIKE ?1
            ORDER BY
                CASE WHEN lower(name) = ?2 THEN 0
                     WHEN lower(name) LIKE ?3 THEN 1
                     ELSE 2 END,
                name
            LIMIT ?4
            "#,
        )
        .map_err(|e| format!("prepare: {e}"))?;

    let rows = stmt
        .query_map(params![pattern, query.to_lowercase(), format!("{}%", query.to_lowercase()), limit as i64], |row| {
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
            })
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("row: {e}"))?);
    }
    Ok(out)
}

/// Obtiene un item por nombre exacto (para tool `buscar_item` con nombre preciso).
pub fn get_item_by_name(conn: &Connection, name: &str) -> Result<Option<ItemRow>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT unique_name, name, description, type, category, subcategory,
                   rarity, polarity, mastery_req, tradable, image_name,
                   build_price, build_quantity, build_time, ingredients, components
            FROM items
            WHERE lower(name) = ?1
               OR lower(unique_name) = ?1
            LIMIT 1
            "#,
        )
        .map_err(|e| format!("prepare: {e}"))?;

    let row = stmt
        .query_row(params![name.to_lowercase()], |row| {
            Ok(ItemRow {
                unique_name: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                item_type: row.get(3)?,
                category: row.get(4)?,
                subcategory: row.get(5)?,
                rarity: row.get(6)?,
                polarity: row.get(6)?,
                mastery_req: row.get(8)?,
                tradable: row.get(9)?,
                image_name: row.get(10)?,
                build_price: row.get(11)?,
                build_quantity: row.get(12)?,
                build_time: row.get(13)?,
                ingredients: row.get(14)?,
                components: row.get(15)?,
            })
        })
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
const WFCD_RAW_BASE: &str =
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
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .user_agent("cehalon-mate/1.0 (+https://github.com/FabianGallardo-coder/Cephalon_Mate)")
        .build()
        .map_err(|e| format!("cliente HTTP: {e}"))?;

    on_progress(8);
    if let Some(items) = fetch_aggregate(&client) {
        println!("[items] {} items del JSON agregado de WFCD", items.len());
        on_progress(50);
        return Ok(items);
    }

    let mut all: Vec<WfcdItem> = Vec::new();
    let total = WFCD_CATEGORIES.len();
    for (i, cat) in WFCD_CATEGORIES.iter().enumerate() {
        let url = format!("{WFCD_RAW_BASE}/{cat}.json");
        let resp = client
            .get(&url)
            .send()
            .map_err(|e| format!("GET {cat}: {e}"))?;
        if !resp.status().is_success() {
            eprintln!("[items] {cat}: HTTP {}", resp.status());
            continue;
        }
        let text = resp.text().map_err(|e| format!("{cat}: response text: {e}"))?;
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
    println!("[items] {} items descargados de WFCD", all.len());
    Ok(all)
}

/// Intenta el JSON agregado de la release (puede no existir).
fn fetch_aggregate(client: &reqwest::blocking::Client) -> Option<Vec<WfcdItem>> {
    println!("[items] probando {WFCD_AGGREGATE_URL}…");
    let resp = client.get(WFCD_AGGREGATE_URL).send().ok()?;
    if !resp.status().is_success() {
        eprintln!("[items] JSON agregado no disponible: HTTP {}", resp.status());
        return None;
    }
    let text = resp.text().ok()?;
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
                    wikia_url = ?16,
                    updated_at = CURRENT_TIMESTAMP
                WHERE unique_name = ?17
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
                    wikia_url
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
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
                ],
            )
            .map_err(|e| format!("insert: {e}"))?;
            inserted += 1;
        }
    }

    on_progress(95);
    tx.commit().map_err(|e| format!("commit: {e}"))?;
    on_progress(100);
    println!("[items] sync OK: {} nuevos, {} actualizados", inserted, updated);
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

    out
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

/// Obtiene el precio de un item desde warframe.market (con caché TTL en SQLite).
/// `item_slug` es el slug de warframe.market (ej. "soma_prime_set", "soma_prime_receiver").
pub fn get_price(conn: &Connection, item_slug: &str) -> Result<PriceCache, String> {
    // 1. Intentar leer del caché si no ha expirado.
    let now = chrono::Utc::now().to_rfc3339();
    let cached: Option<PriceCache> = conn
        .query_row(
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
        .map_err(|e| format!("query cache: {e}"))?;

    if let Some(c) = cached {
        return Ok(c);
    }

    // 2. Caché expirado o inexistente → fetch a warframe.market.
    fetch_and_cache_price(conn, item_slug)
}

fn fetch_and_cache_price(conn: &Connection, item_slug: &str) -> Result<PriceCache, String> {
    let url = format!("https://api.warframe.market/v2/orders/item/{item_slug}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client: {e}"))?;

    let resp = client
        .get(&url)
        .header("accept", "application/json")
        .send()
        .map_err(|e| format!("HTTP GET: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), url));
    }

    let data: OrdersResponse = resp
        .json()
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

    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::seconds(PRICE_TTL_SECS);

    let cache = PriceCache {
        item_slug: item_slug.to_string(),
        item_name,
        sell_min,
        sell_max,
        sell_avg,
        buy_max,
        buy_min,
        buy_avg,
        volume_24h: Some(volume_24h),
        updated_at: now.to_rfc3339(),
        expires_at: expires_at.to_rfc3339(),
    };

    // Upsert en caché.
    conn.execute(
        r#"
        INSERT INTO prices (item_slug, item_name, sell_min, sell_max, sell_avg,
                            buy_max, buy_min, buy_avg, volume_24h, updated_at, expires_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
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
            &cache.item_slug,
            &cache.item_name,
            &cache.sell_min,
            &cache.sell_max,
            &cache.sell_avg,
            &cache.buy_max,
            &cache.buy_min,
            &cache.buy_avg,
            &cache.volume_24h,
            &cache.updated_at,
            &cache.expires_at,
        ],
    )
    .map_err(|e| format!("upsert cache: {e}"))?;

    Ok(cache)
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
}
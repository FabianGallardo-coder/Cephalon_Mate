//! Enrutado por intención: detecta qué herramienta usar según la pregunta.
//!
//! Los modelos pequeños (3B) no siempre llaman a las tools por su cuenta y
//! acaban alucinando polaridades o drops. Este módulo las ejecuta **antes**
//! de la respuesta para que el modelo siempre reciba datos verificados.
//!
//! La detección es conservadora: si no hay una intención clara o no se
//! encuentra ningún item conocido en la BD, no se fuerza nada y el modelo
//! decide con sus propias tools.

use crate::brain::chat::ChatMessage;
use crate::brain::items;
use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

/// Intención detectada: qué tool ejecutar y con qué argumentos.
#[derive(Debug, Clone)]
pub struct Intent {
    /// Nombre de la tool (`buscar_item`, `donde_encontrar`, …).
    pub tool: String,
    /// Argumentos ya listos para `tools::execute`.
    pub args: serde_json::Value,
    /// Etiqueta legible para la línea de la UI (`donde_encontrar(nombre=Soma)`).
    pub label: String,
}

/// Palabras que nunca son un nombre de item.
const STOPWORDS: &[&str] = &[
    "donde", "dónde", "where", "dropea", "dropean", "drop", "drops", "deja", "sacan", "sale",
    "sale", "consigo", "conseguir", "encontrar", "find", "get", "obtain", "farm", "farmeo",
    "farmear", "farming", "mods", "mod", "warframe", "warframes", "arma", "armas", "weapon",
    "weapon", "item", "items", "objeto", "polaridad", "polarity", "polaridades", "stats",
    "estadisticas", "habilidades", "abilities", "info", "informacion", "cuanto", "cuánto",
    "vale", "precio", "price", "platina", "platinum", "para", "por", "con", "sin", "the",
    "that", "this", "what", "which", "who", "how", "can", "you", "i", "me", "my", "is", "are",
    "tenno", "he", "it", "tell", "hablame", "háblame", "info", "sobre", "de", "del", "la", "el",
    "los", "las", "un", "una", "unos", "unas", "y", "o", "a", "al", "en", "que", "qué", "son",
    "tiene", "tienen", "va", "van", "sirven", "use", "usar", "mejor", "buenos", "buenos",
    "prime", "set", "forma", "neuro", "neurode", "neuroptics", "chassis", "systems",
    "blueprint", "rotation", "vault", "requiem", "riven", "duviri", "galatina", "umbra",
    "ordis", "suda", "simaris", "lotus", "tenno", "nightwave",
];

/// Palabras que indican que el usuario quiere saber **dónde obtener** algo.
const WHERE_WORDS: &[&str] = &[
    "dropea", "dropean", "drop", "drops", "consigo", "conseguir", "encontrar", "find", "where",
    "obten", "obtener", "get", "sale", "sacan", "deja", "farm", "farmeo", "farmear", "farming",
    "grind", "location", "lugar", "fuente", "fuentes",
];

/// Palabras que indican que el usuario quiere las **fichas técnicas**.
///
/// Ojo: no se incluyen palabras genéricas como "qué" o "info" sueltas que
/// aparecen en cualquier frase ("¿qué tal estás?") — la detección debe ser
/// conservadora para no activar el prefetch en una conversación normal.
const INFO_WORDS: &[&str] = &[
    "stats", "estadisticas", "estadísticas", "habilidades", "abilities", "informacion",
    "información", "detalles", "cual", "cuál", "cuanto", "cuánto", "vida", "escudo", "armadura",
    "poder", "polaridad", "polarity", "polaridades", "pasiva", "descripcion", "descripción",
    "description", "caracteristicas", "características", "ficha",
];

/// Detecta la intención de la pregunta y devuelve la primera tool a ejecutar.
///
/// Devuelve `None` si no hay intención clara (en ese caso el modelo decide
/// con sus propias tools).
#[cfg(test)]
pub fn detect(conn: &Connection, message: &str) -> Option<Intent> {
    detect_all(conn, message).into_iter().next()
}

/// Detecta **todas** las tools que la pregunta necesita.
///
/// Una pregunta puede pedir datos de varios items ("¿qué mods van en Nova y
/// dónde consigo el Hornet Prime?"); se ejecuta una tool por item.
pub fn detect_all(conn: &Connection, message: &str) -> Vec<Intent> {
    let lower = message.to_lowercase();
    let candidates = item_candidates(conn, message);
    if candidates.is_empty() {
        return Vec::new();
    }

    let wants_mods = has_any(&lower, &["mods", "mod", "compatible", "compatibles", "fit"]);
    let wants_where = has_any(&lower, WHERE_WORDS);
    let wants_info = has_any(&lower, INFO_WORDS);

    let mut out = Vec::new();
    for name in candidates.into_iter().take(3) {
        let (tool, args) = if wants_mods || wants_where {
            let tipo = item_kind(conn, &name);
            ("donde_encontrar", json!({ "nombre": name.clone(), "tipo": tipo }))
        } else if wants_info {
            ("buscar_item", json!({ "nombre": name.clone() }))
        } else {
            // Sin señal clara: no forzamos la tool.
            return Vec::new();
        };

        out.push(Intent {
            tool: tool.to_string(),
            args: args.clone(),
            label: format!("{tool}(nombre={name})"),
        });
    }
    out
}

/// ¿La pregunta pide datos del juego (drop, polaridad, stats, mods)?
///
/// Se usa para inyectar el "guard" anti-alucinación aunque no haya ningún
/// item conocido en la BD: sin ese aviso el modelo inventa recetas y drops
/// para items que no existen en el dataset (p. ej. "Hornet Prime", que
/// upstream WFCD ya no publica).
pub fn asks_game_data(message: &str) -> bool {
    let lower = message.to_lowercase();
    has_any(&lower, WHERE_WORDS)
        || has_any(&lower, INFO_WORDS)
        || has_any(&lower, &["mods", "mod", "compatible", "compatibles", "fit"])
}

/// Sufijos que forman parte del nombre del item ("Hornet" + "Prime").
const NAME_SUFFIXES: &[&str] = &[
    "prime", "set", "deluxe", "umbra", "vex", "vandal", "wraith", "prisma", "origami", "reaper",
    "fossa", "pax", "kavat", "riven", "epitoma", "stellar", "carnifex", "blazeframe", "mk1",
];

/// Nombres que **parecen** items pero no están en la base de datos.
///
/// Se detectan por capitalización (los items de Warframe se escriben en
/// inglés con mayúscula) y se descartan los verbos de pregunta. El resultado
/// se usa para avisar al modelo de que ese item no existe en el dataset, en
/// lugar de dejarlo inventar datos.
pub fn unknown_items(conn: &Connection, message: &str) -> Vec<String> {
    let words: Vec<&str> = message
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .filter(|w| !w.is_empty())
        .collect();

    let mut out: Vec<String> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let lw = w.to_lowercase();
        if lw.len() < 4 || STOPWORDS.contains(&lw.as_str()) {
            continue;
        }
        // Solo palabras con inicial mayúscula y resto en minúsculas.
        let Some(first) = w.chars().next() else { continue };
        if !first.is_uppercase() || w.chars().skip(1).any(|c| c.is_uppercase()) {
            continue;
        }
        if canonical_name(conn, w).is_some() {
            continue;
        }
        // ¿Es un verbo/pregunta común? → no es un item.
        if QUESTION_WORDS.contains(&lw.as_str()) {
            continue;
        }

        // Une sufijos: "Hornet Prime" (Prime está en STOPWORDS).
        let mut name = w.to_string();
        if let Some(next) = words.get(i + 1) {
            let ln = next.to_lowercase();
            if NAME_SUFFIXES.contains(&ln.as_str()) {
                name.push(' ');
                name.push_str(next);
            }
        }
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Palabras que empiezan por mayúscula pero NO son items.
const QUESTION_WORDS: &[&str] = &[
    "dime", "busca", "buscar", "buscame", "necesito", "quiero", "cuentame", "cuéntame",
    "hablame", "háblame", "explicame", "explícame", "ayuda", "favor", "about", "give", "show",
    "list", "explain", "describe", "thanks", "gracias", "hola", "hello", "hey", "please",
    "cuanto", "cuánto", "cual", "cuál", "donde", "dónde", "cuando", "cuándo", "quien", "quién",
    "puedes", "puede", "ayudame", "ayúdame", "necesitaria", "necesitaría", "recomiendame",
    "recomiéndame", "recomienda", "mejor", "mejores",
];

/// Tipo de un item según la BD: `mod`, `warframe` o `item`.
fn item_kind(conn: &Connection, name: &str) -> &'static str {
    let ty = conn
        .query_row(
            "SELECT lower(type) FROM items WHERE lower(name) = ?1 LIMIT 1",
            rusqlite::params![name.to_lowercase()],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .unwrap_or_default();

    if ty.contains("mod") {
        "mod"
    } else if ty.contains("warframe") {
        "warframe"
    } else {
        "item"
    }
}

/// Candidatos de nombres de item presentes en el mensaje y en la BD.
///
/// Prioriza lo que el usuario escribió capitalizado (los nombres de Warframe
/// son en inglés y el usuario suele escribirlos con mayúscula).
fn item_candidates(conn: &Connection, message: &str) -> Vec<String> {
    let words: Vec<&str> = message
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .filter(|w| !w.is_empty())
        .collect();

    let mut scored: Vec<(usize, String)> = Vec::new();

    for (i, w) in words.iter().enumerate() {
        let lw = w.to_lowercase();
        if lw.len() < 3 || STOPWORDS.contains(&lw.as_str()) {
            continue;
        }
        // ¿Existe en la BD como nombre exacto? (se usa el nombre canónico)
        let Some(canonical) = canonical_name(conn, w) else {
            continue;
        };
        // Prioridad: mayúscula inicial (3) > resto (1); los bigramas +4.
        let score = if w.chars().next().is_some_and(|c| c.is_uppercase()) {
            3
        } else {
            1
        };
        if i + 1 < words.len() {
            let bigram = format!("{} {}", w, words[i + 1]);
            if let Some(big) = canonical_name(conn, &bigram) {
                scored.push((score + 4, big));
                continue;
            }
        }
        scored.push((score, canonical));
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<String> = Vec::new();
    for (_, name) in scored {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Nombre canónico en la BD de un item (case-insensitive), si existe.
fn canonical_name(conn: &Connection, name: &str) -> Option<String> {
    conn.query_row(
        "SELECT name FROM items WHERE lower(name) = ?1 LIMIT 1",
        rusqlite::params![name.to_lowercase()],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// ¿El item es un warframe?
#[allow(dead_code)]
fn looks_like_warframe(conn: &Connection, name: &str) -> bool {
    item_kind(conn, name) == "warframe"
}

/// ¿El mensaje contiene alguna de las palabras clave?
fn has_any(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| {
        // Busca como palabra completa (no como subcadena).
        text.split(|c: char| !c.is_alphanumeric())
            .any(|t| t == *w)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::items::create_schema;
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO items (unique_name, name, type, category) VALUES ('/w/r', 'Rhino', 'Warframe', 'Warframes')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO items (unique_name, name, type, category, polarity) VALUES ('/m/s', 'Serration', 'Primary Mod', 'Mods', 'madurai')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn detects_where_to_find_for_mod() {
        let conn = db();
        let intent = detect(&conn, "¿Dónde dropea Serration?").expect("intent");
        assert_eq!(intent.tool, "donde_encontrar");
        assert_eq!(intent.args["nombre"], "Serration");
        assert_eq!(intent.args["tipo"], "mod");
    }

    #[test]
    fn detects_where_to_find_for_warframe() {
        let conn = db();
        let intent = detect(&conn, "Where can I farm Rhino?").expect("intent");
        assert_eq!(intent.tool, "donde_encontrar");
        assert_eq!(intent.args["nombre"], "Rhino");
        assert_eq!(intent.args["tipo"], "warframe");
    }

    #[test]
    fn detects_mods_for_warframe() {
        let conn = db();
        let intent = detect(&conn, "¿Qué mods van en Rhino?").expect("intent");
        assert_eq!(intent.tool, "donde_encontrar");
        assert_eq!(intent.args["tipo"], "warframe");
    }

    #[test]
    fn detects_info_request() {
        let conn = db();
        let intent = detect(&conn, "Háblame de las stats de Rhino").expect("intent");
        assert_eq!(intent.tool, "buscar_item");
        assert_eq!(intent.args["nombre"], "Rhino");
    }

    #[test]
    fn ignores_unknown_items() {
        let conn = db();
        assert!(detect(&conn, "¿Dónde dropea Objeto Inventado?").is_none());
    }

    #[test]
    fn ignores_questions_without_intent() {
        let conn = db();
        // "Rhino" aparece, pero no hay señal de qué quiere saber.
        assert!(detect(&conn, "hola qué tal").is_none());
    }

    #[test]
    fn candidates_prefer_capitalized_names() {
        let conn = db();
        let cands = item_candidates(&conn, "hola Rhino y serration más");
        assert_eq!(cands[0], "Rhino");
        assert!(cands.contains(&"Serration".to_string()));
    }

    #[test]
    fn detects_unknown_item_with_suffix() {
        let conn = db();
        // "Hornet Prime" no está en la BD (upstream WFCD ya no lo publica).
        let u = unknown_items(&conn, "Where can I get Hornet Prime?");
        assert!(
            u.iter().any(|x| x == "Hornet Prime"),
            "no se detectó Hornet Prime: {u:?}"
        );
    }

    #[test]
    fn known_items_are_not_unknown() {
        let conn = db();
        assert!(
            unknown_items(&conn, "Where does Serration drop?").is_empty(),
            "Serration está en la BD"
        );
        assert!(
            unknown_items(&conn, "Tell me about Rhino").is_empty(),
            "Rhino está en la BD"
        );
    }

    #[test]
    fn question_words_are_not_items() {
        let conn = db();
        // "Háblame" / "Tell" son verbos, no items.
        assert!(unknown_items(&conn, "Háblame de Rhino, por favor").is_empty());
    }

    #[test]
    fn detects_game_data_questions() {
        assert!(asks_game_data("Where do I get Hornet Prime?"));
        assert!(asks_game_data("Háblame de las stats de Rhino"));
        assert!(asks_game_data("¿Qué mods van en Nova?"));
        assert!(!asks_game_data("hola Tenno"));
        assert!(!asks_game_data("¿qué tal estás?"));
    }

    #[test]
    fn prefetch_intents_works() {
        let conn = db();
        items::set_items_db(conn);
        let msgs = vec![ChatMessage::user("¿Dónde dropea Serration?")];
        let intents = prefetch_intents(&msgs);
        assert!(!intents.is_empty());
        assert_eq!(intents[0].tool, "donde_encontrar");
    }
}

/// Detecta las intenciones del último mensaje del usuario (versión pública para GUI).
///
/// Devuelve las tools a ejecutar si la pregunta es claramente sobre datos del
/// juego y hay items conocidos en la BD.
pub fn prefetch_intents(messages: &[ChatMessage]) -> Vec<Intent> {
    let Some(last_user) = messages.iter().rev().find(|m| m.role == "user") else {
        return Vec::new();
    };
    let Some(arc) = items::get_items_db() else {
        return Vec::new();
    };
    let intents = {
        let conn = match arc.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        detect_all(&conn, &last_user.content)
    };
    intents
}

/// Detecta items mencionados por el usuario que **no** están en la BD (versión pública para GUI).
///
/// Sirve para advertir al modelo de que ese item no existe en el dataset
/// (y que no debe inventar datos sobre él).
pub fn prefetch_unknowns(messages: &[ChatMessage]) -> Vec<String> {
    let Some(last_user) = messages.iter().rev().find(|m| m.role == "user") else {
        return Vec::new();
    };
    let Some(arc) = items::get_items_db() else {
        return Vec::new();
    };
    let unknowns = {
        let conn = match arc.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        unknown_items(&conn, &last_user.content)
    };
    // Sólo tiene sentido avisar si la pregunta pide datos del juego.
    if asks_game_data(&last_user.content) {
        unknowns.into_iter().take(3).collect()
    } else {
        Vec::new()
    }
}

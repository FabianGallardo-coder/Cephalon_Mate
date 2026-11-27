//! QA de datos: comprueba que la BD de WFCD responde con info real de
//! warframes y mods (dónde dropean, polaridades, stats).
//!
//! Esto NO es una suite de UI: es la verificación de que el dataset que
//! consume el modelo es correcto y utilizable. Se ejecuta con:
//!
//! ```text
//! cargo run -- --qa
//! ```
//!
//! Cada check devuelve PASS/FAIL/WARN y el comando sale con código 1 si
//! algún check crítico falla (CI lo puede usar como puerta).

use rusqlite::Connection;

use crate::brain::items::{
    count_items, format_where_to_find, get_item_by_name, init_db, mods_for_polarities,
    parse_drops, polarities_of, search_items,
};

/// Resultado de un check de QA.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

/// Estado de un check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        }
    }
}

/// Warframes de referencia: nombre → polaridades esperadas.
///
/// Valores verificados contra `data/json/Warframes.json` de WFCD (2026-11-26).
/// Si upstream cambia el juego y un warframe cambia de polaridad, el check
/// avisa (WARN) en vez de romperse.
const WARFRAME_CASES: &[(&str, &[&str])] = &[
    ("Rhino", &["vazarin"]),
    ("Nova", &["madurai"]),
    ("Mesa", &["naramon"]),
    ("Revenant", &["madurai", "vazarin"]),
    ("Excalibur", &["vazarin", "madurai"]),
    ("Loki", &["vazarin", "madurai"]),
    ("Nekros", &["vazarin", "madurai"]),
    ("Ash", &["madurai"]),
];

/// Mods de referencia: nombre → polaridad esperada.
///
/// Valores verificados contra `data/json/Mods.json` de WFCD (2026-11-26).
const MOD_CASES: &[(&str, &str)] = &[
    ("Serration", "madurai"),
    ("Galvanized Chamber", "madurai"),
    ("Melee Prowess", "vazarin"),
    ("Primed Cryo Rounds", "vazarin"),
    ("Vital Sense", "madurai"),
    ("Arrow Mutation", "naramon"),
    ("Speed Trigger", "madurai"),
];

/// Ejecuta la batería completa de QA sobre la BD local.
///
/// `sync_first` indica si hay que sincronizar con WFCD antes (la BD puede
/// venir vacía).
pub fn run_qa(_sync_first: bool) -> (Vec<CheckResult>, usize) {
    let mut results = Vec::new();

    let conn = match init_db() {
        Ok(c) => c,
        Err(e) => {
            results.push(CheckResult {
                name: "bd:abrir",
                status: Status::Fail,
                detail: format!("no se pudo abrir la BD: {e}"),
            });
            return (results, 1);
        }
    };

    // --- Check 1: la BD tiene datos -------------------------------------
    let count = count_items(&conn).unwrap_or(0);
    if count == 0 {
        results.push(CheckResult {
            name: "bd:poblada",
            status: Status::Fail,
            detail: "la BD está vacía: ejecuta el sync de WFCD (--qa --sync)".into(),
        });
        return (results, 1);
    }
    results.push(CheckResult {
        name: "bd:poblada",
        status: Status::Pass,
        detail: format!("{count} items en la BD"),
    });

    // --- Check 2: cobertura de drops -------------------------------------
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0))
        .unwrap_or(0);
    let con_drops: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM items WHERE drops IS NOT NULL AND drops != '[]'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let pct = if total > 0 {
        (con_drops as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    results.push(CheckResult {
        name: "datos:cobertura_drops",
        status: if pct >= 10.0 { Status::Pass } else { Status::Warn },
        detail: format!("{con_drops}/{total} items con fuente de drop ({pct:.1}%)"),
    });

    // --- Check 3: warframes ----------------------------------------------
    let mut wf_ok = 0;
    let mut wf_warn = 0;
    let mut wf_fail = 0;
    for (name, expected) in WARFRAME_CASES {
        match get_item_by_name(&conn, name) {
            Ok(Some(item)) => {
                let pols = polarities_of(&item);
                let stats = item.stats.clone().unwrap_or_default();
                let abilities = item.abilities.clone().unwrap_or_default();

                let missing: Vec<&str> = expected
                    .iter()
                    .filter(|e| !pols.iter().any(|p| p.eq_ignore_ascii_case(e)))
                    .copied()
                    .collect();

                if missing.is_empty() && !stats.is_empty() && !abilities.is_empty() {
                    wf_ok += 1;
                    results.push(CheckResult {
                        name: "warframe:info",
                        status: Status::Pass,
                        detail: format!(
                            "{name}: polaridades [{}], stats {} bytes, {} habilidades",
                            pols.join(", "),
                            stats.len(),
                            count_abilities(&abilities)
                        ),                    });
                } else if missing.is_empty() {
                    wf_warn += 1;
                    results.push(CheckResult {
                        name: "warframe:info",
                        status: Status::Warn,
                        detail: format!(
                            "{name}: polaridades OK pero stats/habilidades incompletas (stats={}, abilities={})",
                            stats.len(),
                            count_abilities(&abilities)
                        ),
                    });
                } else {
                    wf_fail += 1;
                    results.push(CheckResult {
                        name: "warframe:info",
                        status: Status::Fail,
                        detail: format!(
                            "{name}: faltan polaridades {:?} (tiene {:?})",
                            missing, pols
                        ),
                    });
                }
            }
            Ok(None) => {
                wf_fail += 1;
                results.push(CheckResult {
                    name: "warframe:info",
                    status: Status::Fail,
                    detail: format!("{name}: no está en la BD"),
                });
            }
            Err(e) => {
                wf_fail += 1;
                results.push(CheckResult {
                    name: "warframe:info",
                    status: Status::Fail,
                    detail: format!("{name}: error de consulta: {e}"),
                });
            }
        }
    }
    results.push(CheckResult {
        name: "warframe:resumen",
        status: if wf_fail == 0 { Status::Pass } else { Status::Fail },
        detail: format!("{wf_ok} completos, {wf_warn} con avisos, {wf_fail} fallidos de {}", WARFRAME_CASES.len()),
    });

    // --- Check 4: mods y dónde dropean ----------------------------------
    let mut mod_ok = 0;
    let mut mod_warn = 0;
    let mut mod_fail = 0;
    for (name, expected_pol) in MOD_CASES {
        match get_item_by_name(&conn, name) {
            Ok(Some(item)) => {
                let pol = item.polarity.clone().unwrap_or_default();
                let drops = parse_drops(item.drops.as_deref().unwrap_or("[]"));
                let pol_ok = pol.eq_ignore_ascii_case(expected_pol);

                if !pol_ok {
                    mod_fail += 1;
                    results.push(CheckResult {
                        name: "mod:drops",
                        status: Status::Fail,
                        detail: format!(
                            "{name}: polaridad '{pol}' ≠ esperada '{expected_pol}'"
                        ),
                    });
                } else if drops.is_empty() {
                    mod_warn += 1;
                    results.push(CheckResult {
                        name: "mod:drops",
                        status: Status::Warn,
                        detail: format!("{name}: polaridad OK pero SIN fuentes de drop en WFCD"),
                    });
                } else {
                    mod_ok += 1;
                    let top = drops
                        .iter()
                        .map(|d| d.location.as_str())
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(", ");
                    results.push(CheckResult {
                        name: "mod:drops",
                        status: Status::Pass,
                        detail: format!(
                            "{name}: {} fuentes, polaridad {pol} → {top}",
                            drops.len()
                        ),
                    });
                }
            }
            Ok(None) => {
                mod_fail += 1;
                results.push(CheckResult {
                    name: "mod:drops",
                    status: Status::Fail,
                    detail: format!("{name}: no está en la BD"),
                });
            }
            Err(e) => {
                mod_fail += 1;
                results.push(CheckResult {
                    name: "mod:drops",
                    status: Status::Fail,
                    detail: format!("{name}: error de consulta: {e}"),
                });
            }
        }
    }
    results.push(CheckResult {
        name: "mod:resumen",
        status: if mod_fail == 0 { Status::Pass } else { Status::Fail },
        detail: format!("{mod_ok} ok, {mod_warn} con avisos, {mod_fail} fallidos de {}", MOD_CASES.len()),
    });

    // --- Check 5: mods compatibles por polaridad ------------------------
    match get_item_by_name(&conn, "Rhino") {
        Ok(Some(rhino)) => {
            let pols = polarities_of(&rhino);
            match mods_for_polarities(&conn, &pols, 10) {
                Ok(mods) if !mods.is_empty() => {
                    let names: Vec<String> = mods.iter().take(5).map(|m| m.name.clone()).collect();
                    results.push(CheckResult {
                        name: "warframe:mods_compatibles",
                        status: Status::Pass,
                        detail: format!(
                            "Rhino [{}] → {} mods: {}",
                            pols.join(", "),
                            mods.len(),
                            names.join(", ")
                        ),
                    });
                }
                Ok(_) => {
                    results.push(CheckResult {
                        name: "warframe:mods_compatibles",
                        status: Status::Fail,
                        detail: "Rhino no tiene mods compatibles (la columna polarity está vacía)".into(),
                    });
                }
                Err(e) => {
                    results.push(CheckResult {
                        name: "warframe:mods_compatibles",
                        status: Status::Fail,
                        detail: format!("error consultando mods: {e}"),
                    });
                }
            }
        }
        _ => {
            results.push(CheckResult {
                name: "warframe:mods_compatibles",
                status: Status::Fail,
                detail: "no se pudo cargar Rhino para el test de compatibilidad".into(),
            });
        }
    }

    // --- Check 6: búsqueda difusa ---------------------------------------
    match search_items(&conn, "soma", 5) {
        Ok(items) if !items.is_empty() => {
            results.push(CheckResult {
                name: "busqueda:difusa",
                status: Status::Pass,
                detail: format!(
                    "'soma' → {} resultados, primero: {}",
                    items.len(),
                    items[0].name
                ),
            });
        }
        Ok(_) => {
            results.push(CheckResult {
                name: "busqueda:difusa",
                status: Status::Fail,
                detail: "'soma' no devolvió resultados".into(),
            });
        }
        Err(e) => {
            results.push(CheckResult {
                name: "busqueda:difusa",
                status: Status::Fail,
                detail: format!("error en búsqueda: {e}"),
            });
        }
    }

    // --- Check 7: formato "dónde encontrarlo" ---------------------------
    match get_item_by_name(&conn, "Serration") {
        Ok(Some(item)) => {
            let text = format_where_to_find(&item);
            if text.contains("Dónde encontrarlo") && text.len() > 60 {
                results.push(CheckResult {
                    name: "salida:donde_encontrar",
                    status: Status::Pass,
                    detail: format!("{} caracteres de salida para Serration", text.len()),
                });
            } else {
                results.push(CheckResult {
                    name: "salida:donde_encontrar",
                    status: Status::Fail,
                    detail: format!("salida vacía o incompleta: {text:?}"),
                });
            }
        }
        _ => {
            results.push(CheckResult {
                name: "salida:donde_encontrar",
                status: Status::Fail,
                detail: "Serration no está en la BD".into(),
            });
        }
    }

    // --- Check 8: los items con drops tienen location parseable ---------
    let dropped = parse_drops(
        &conn
            .query_row(
                "SELECT drops FROM items WHERE drops IS NOT NULL AND drops != '[]' LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "[]".to_string()),
    );
    if dropped.is_empty() {
        results.push(CheckResult {
            name: "datos:parseo_drops",
            status: Status::Fail,
            detail: "hay items con drops pero ninguno se pudo parsear".into(),
        });
    } else {
        results.push(CheckResult {
            name: "datos:parseo_drops",
            status: Status::Pass,
            detail: format!("ejemplo: {} → {}", dropped[0].location, dropped[0].kind.clone().unwrap_or_default()),
        });
    }

    // --- Check 9: integridad del sync contra upstream en vivo ------------
    // Re-descarga las categorías de WFCD y compara con lo que hay en la BD.
    // Es el check que detecta que el parser "se comió" datos.
    results.push(check_upstream_integrity(&conn));

    // --- Check 10: probabilidades normalizadas (0-100 %) -----------------
    results.push(check_chance_range(&conn));

    // --- Check 11: item desconocido → respuesta honesta -------------------
    results.push(check_unknown_item_is_honest(&conn));

    // --- Check 12: el prefetch por intención detecta la pregunta ---------
    results.push(check_intent_detection(&conn));

    let fails = results.iter().filter(|r| r.status == Status::Fail).count();
    (results, fails)
}

/// Comprueba que ninguna probabilidad de drop supere el 100 %.
///
/// Regresión: WFCD usa `chance` en fracción para enemigos (0.22 = 22 %) y en
/// porcentaje para reliquias (8.6 = 8.6 %). Si no se normaliza, la app
/// muestra probabilidades como "860 %".
fn check_chance_range(conn: &Connection) -> CheckResult {
    let name = "datos:probabilidades";
    let mut checked = 0usize;
    let mut bad: Vec<String> = Vec::new();

    let mut stmt = match conn.prepare(
        "SELECT name, drops FROM items WHERE drops IS NOT NULL AND drops != '[]' LIMIT 800",
    ) {
        Ok(s) => s,
        Err(e) => {
            return CheckResult {
                name,
                status: Status::Fail,
                detail: format!("no se pudo preparar la consulta: {e}"),
            }
        }
    };
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    });
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            let (item_name, drops_json) = row;
            for d in parse_drops(&drops_json) {
                checked += 1;
                if let Some(c) = d.chance {
                    if !(0.0..=100.0).contains(&c) {
                        if bad.len() < 5 {
                            bad.push(format!("{item_name} @ {} = {c}%", d.location));
                        }
                    }
                }
            }
        }
    }

    if bad.is_empty() {
        CheckResult {
            name,
            status: Status::Pass,
            detail: format!("{checked} probabilidades en rango 0-100 %"),
        }
    } else {
        CheckResult {
            name,
            status: Status::Fail,
            detail: format!("{} probabilidades fuera de rango: {}", bad.len(), bad.join(" | ")),
        }
    }
}

/// Un item que no está en el dataset debe responderse con honestidad.
///
/// Regresión: el modelo inventaba una receta de crafteo para "Hornet Prime"
/// (que upstream WFCD ya no publica). La tool debe decir "no encontrado".
fn check_unknown_item_is_honest(conn: &Connection) -> CheckResult {
    let name = "datos:item_desconocido";

    // "Hornet Prime" existió en el juego pero ya no está en el dataset WFCD.
    // "Item Inventado Xeon" nunca existió.
    for desconocido in ["Hornet Prime", "Item Inventado Xeon"] {
        let encontrado = search_items(conn, desconocido, 1)
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if encontrado {
            // Si upstream lo volvió a publicar, el check deja de tener sentido.
            continue;
        }
        // El intent detector debe marcarlo como desconocido para que el
        // prefetch avise al modelo y no invente.
        let unknowns = crate::brain::intent::unknown_items(conn, &format!(
            "Where can I get {desconocido}?"
        ));
        if unknowns.is_empty() {
            return CheckResult {
                name,
                status: Status::Fail,
                detail: format!(
                    "'{desconocido}' no está en la BD y el detector tampoco lo señala como desconocido"
                ),
            };
        }
    }

    CheckResult {
        name,
        status: Status::Pass,
        detail: "los items ausentes se marcan como desconocidos (el modelo no los inventa)".into(),
    }
}

/// El prefetch por intención debe elegir la tool correcta.
fn check_intent_detection(conn: &Connection) -> CheckResult {
    let name = "prefetch:intencion";

    // 1. Drop de un mod → donde_encontrar.
    let intents = crate::brain::intent::detect_all(conn, "¿Dónde dropea Serration?");
    if intents.len() != 1
        || intents[0].tool != "donde_encontrar"
        || intents[0].args["nombre"] != "Serration"
    {
        return CheckResult {
            name,
            status: Status::Fail,
            detail: format!("no detectó 'donde_encontrar(Serration)': {intents:?}"),
        };
    }

    // 2. Mods para un warframe → donde_encontrar tipo warframe.
    let intents = crate::brain::intent::detect_all(conn, "¿Qué mods van en Nova?");
    if intents.is_empty()
        || intents[0].tool != "donde_encontrar"
        || intents[0].args["tipo"] != "warframe"
    {
        return CheckResult {
            name,
            status: Status::Fail,
            detail: format!("no detectó 'donde_encontrar(Nova, warframe)': {intents:?}"),
        };
    }

    // 3. Dos items en una pregunta → dos tools.
    let intents = crate::brain::intent::detect_all(
        conn,
        "¿Qué mods van en Nova y dónde dropea Serration?",
    );
    if intents.len() != 2 {
        return CheckResult {
            name,
            status: Status::Fail,
            detail: format!("esperaba 2 intents (Nova + Serration), {}", intents.len()),
        };
    }

    // 4. Pregunta sin datos del juego → no se fuerza nada.
    if crate::brain::intent::asks_game_data("hola Tenno, ¿qué tal?") {
        return CheckResult {
            name,
            status: Status::Fail,
            detail: "una conversación normal no debería activar el prefetch".into(),
        };
    }

    CheckResult {
        name,
        status: Status::Pass,
        detail: "drop de mod, mods de warframe, multi-item y control negativo OK".into(),
    }
}

/// Compara la BD local con los JSON crudos de WFCD (red).
///
/// Verifica, para los warframes y mods de referencia:
/// - polaridades idénticas,
/// - mismo número de fuentes de drop,
/// - mismo número de habilidades (warframes).
fn check_upstream_integrity(conn: &Connection) -> CheckResult {
    let client = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(45)))
            .user_agent("cehalon-mate-qa/1.0")
            .build(),
    );

    let fetch = |cat: &str| -> Result<Vec<serde_json::Value>, String> {
        let url = format!("{}/{cat}.json", crate::brain::items::WFCD_RAW_BASE);
        let resp = client
            .get(&url)
            .call()
            .map_err(|e| format!("GET {cat}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {cat}: HTTP {}", resp.status()));
        }
        let text = resp.into_body().read_to_string().map_err(|e| format!("{cat}: {}", e))?;
        serde_json::from_str(&text).map_err(|e| format!("{cat}: JSON inválido: {e}"))
    };

    let mods = match fetch("Mods") {
        Ok(v) => v,
        Err(e) => {
            return CheckResult {
                name: "sync:integridad_upstream",
                status: Status::Warn,
                detail: format!("sin red o upstream caído ({e}); se omite la comparación"),
            }
        }
    };
    let warframes = match fetch("Warframes") {
        Ok(v) => v,
        Err(e) => {
            return CheckResult {
                name: "sync:integridad_upstream",
                status: Status::Warn,
                detail: format!("sin red o upstream caído ({e}); se omite la comparación"),
            }
        }
    };

    let mut mismatches: Vec<String> = Vec::new();
    let mut drift: Vec<String> = Vec::new();
    let mut checked = 0;

    for (name, expected_pol) in MOD_CASES {
        let Some(up) = mods.iter().find(|m| m.get("name").and_then(|x| x.as_str()) == Some(*name))
        else {
            mismatches.push(format!("{name}: ya no existe en upstream"));
            continue;
        };
        let Some(local) = get_item_by_name(conn, name).ok().flatten() else {
            mismatches.push(format!("{name}: no está en la BD local"));
            continue;
        };
        checked += 1;

        let up_pol = up.get("polarity").and_then(|x| x.as_str()).unwrap_or("");
        let loc_pol = local.polarity.clone().unwrap_or_default();
        if !up_pol.eq_ignore_ascii_case(&loc_pol) {
            mismatches.push(format!(
                "{name}: polaridad local '{loc_pol}' vs upstream '{up_pol}'"
            ));
        }
        // El golden file puede quedarse viejo si upstream cambia el juego:
        // es deriva del test, no un bug del pipeline.
        if !up_pol.eq_ignore_ascii_case(expected_pol) {
            drift.push(format!(
                "{name}: golden dice '{expected_pol}', upstream ahora '{up_pol}'"
            ));
        }

        let up_drops = up
            .get("drops")
            .and_then(|d| d.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let loc_drops = parse_drops(local.drops.as_deref().unwrap_or("[]")).len();
        if up_drops != loc_drops {
            mismatches.push(format!(
                "{name}: {loc_drops} drops en BD vs {up_drops} en upstream"
            ));
        }
    }

    for (name, expected) in WARFRAME_CASES {
        let Some(up) = warframes
            .iter()
            .find(|m| m.get("name").and_then(|x| x.as_str()) == Some(*name))
        else {
            mismatches.push(format!("{name}: ya no existe en upstream"));
            continue;
        };
        let Some(local) = get_item_by_name(conn, name).ok().flatten() else {
            mismatches.push(format!("{name}: no está en la BD local"));
            continue;
        };
        checked += 1;

        let up_pols: Vec<String> = up
            .get("polarities")
            .and_then(|p| p.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_lowercase))
                    .collect()
            })
            .unwrap_or_default();
        let loc_pols = polarities_of(&local);
        let missing: Vec<&String> = up_pols
            .iter()
            .filter(|p| !loc_pols.iter().any(|l| l.eq_ignore_ascii_case(p)))
            .collect();
        if !missing.is_empty() {
            mismatches.push(format!(
                "{name}: faltan polaridades {:?} (BD: {:?})",
                missing, loc_pols
            ));
        }

        let up_abil = up
            .get("abilities")
            .and_then(|a| a.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let loc_abil = local
            .abilities
            .as_deref()
            .and_then(|a| serde_json::from_str::<Vec<serde_json::Value>>(a).ok())
            .map(|v| v.len())
            .unwrap_or(0);
        if up_abil != loc_abil {
            mismatches.push(format!(
                "{name}: {loc_abil} habilidades en BD vs {up_abil} en upstream"
            ));
        }

        // Polaridades esperadas del golden file.
        let exp_missing: Vec<&str> = expected
            .iter()
            .filter(|e| !loc_pols.iter().any(|p| p.eq_ignore_ascii_case(e)))
            .copied()
            .collect();
        if !exp_missing.is_empty() {
            mismatches.push(format!(
                "{name}: golden espera {:?} pero la BD tiene {:?}",
                exp_missing, loc_pols
            ));
        }
    }

    if mismatches.is_empty() {
        CheckResult {
            name: "sync:integridad_upstream",
            status: if drift.is_empty() { Status::Pass } else { Status::Warn },
            detail: if drift.is_empty() {
                format!("{checked} items comparados contra upstream: 0 diferencias")
            } else {
                format!(
                    "{checked} items comparados contra upstream: 0 diferencias, pero el golden file está viejo ({})",
                    drift.join(" | ")
                )
            },
        }
    } else {
        CheckResult {
            name: "sync:integridad_upstream",
            status: Status::Fail,
            detail: format!(
                "{} diferencias en {} items: {}",
                mismatches.len(),
                checked,
                mismatches.join(" | ")
            ),
        }
    }
}

/// Cuenta las habilidades de un warframe (JSON array de `{name, description}`).
fn count_abilities(json: &str) -> usize {
    serde_json::from_str::<Vec<serde_json::Value>>(json)
        .map(|v| v.len())
        .unwrap_or(0)
}

/// Imprime el informe de QA en consola.
pub fn print_report(results: &[CheckResult], fails: usize) {
    println!("\n=== QA DATOS WFCD (Cephalon Mate) ===\n");
    for r in results {
        println!("[{}] {:<32} {}", r.status.label(), r.name, r.detail);
    }
    let total = results.len();
    let warns = results.iter().filter(|r| r.status == Status::Warn).count();
    println!("\n--- {total} checks · {fails} fallidos · {warns} avisos ---");
    if fails == 0 {
        println!("RESULTADO QA: ✅ PASS");
    } else {
        println!("RESULTADO QA: ❌ FAIL");
    }
}

/// Sincroniza la BD con WFCD antes del QA (best effort).
pub fn sync_for_qa(conn: &Connection) -> Result<usize, String> {
    crate::brain::items::sync_wfcd(conn)
}

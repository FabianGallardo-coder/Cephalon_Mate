//! Fetch real de worldstate de Warframe desde api.warframestat.us.
//!
//! P08 del `PLAN_EJECUCION.md`: thread background con `mpsc`, refresh configurable.

use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Estado del worldstate cacheado (protegido por el thread dueño).
#[derive(Debug, Clone)]
pub struct WorldState {
    pub fissures: Vec<Fissure>,
    pub alerts: Vec<Alert>,
    pub sortie: Option<Sortie>,
    pub invasions: Vec<Invasion>,
    pub fetched_at: Instant,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            fissures: Vec::new(),
            alerts: Vec::new(),
            sortie: None,
            invasions: Vec::new(),
            fetched_at: Instant::now().checked_sub(Duration::from_secs(3600)).unwrap_or_else(Instant::now),
        }
    }
}

/// Thread que refresca el worldstate en background.
pub fn spawn_fetcher(refresh_secs: u64) -> Receiver<WorldState> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let client = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(10)))
                .build(),
        );
        let interval = Duration::from_secs(refresh_secs);
        let mut last_fetch = Instant::now().checked_sub(interval).unwrap_or_else(Instant::now); // forzar fetch inmediato

        loop {
            let now = Instant::now();
            if now.duration_since(last_fetch) >= interval {
                match fetch_all(&client) {
                    Ok(ws) => {
                        if tx.send(ws).is_err() {
                            break; // receptor caído → terminar thread
                        }
                        last_fetch = Instant::now();
                    }
                    Err(_) => {
                        // fetch fallido: reintentar en el siguiente ciclo sin loguear
                        // (el log corrompería la TUI en raw mode)
                    }
                }
            }
            thread::sleep(Duration::from_secs(5)); // poll cada 5s
        }
    });

    rx
}

/// Fetch puntual del worldstate (comando `/worldstate`).
///
/// Devuelve el estado ya listo para pintar: el TUI lo llama desde un thread
/// aparte para no congelar la UI.
pub fn fetch_now() -> Result<WorldState, String> {
    let client = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    );
    fetch_all(&client)
}

fn fetch_all(client: &ureq::Agent) -> Result<WorldState, String> {
    let fissures = fetch_fissures(client)?;           // imprescindible
    let alerts = fetch_alerts(client).unwrap_or_default();
    let sortie = fetch_sortie(client).unwrap_or_default();
    let invasions = fetch_invasions(client).unwrap_or_default();

    Ok(WorldState {
        fissures,
        alerts,
        sortie,
        invasions,
        fetched_at: Instant::now(),
    })
}

// ── Modelos de la API warframestat.us ──────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FissureRaw {
    #[serde(rename = "tier")]
    tier: String,
    #[serde(rename = "node")]
    node: String,
    #[serde(rename = "missionType")]
    mission_type: String,
    #[serde(rename = "enemy")]
    enemy: String,
    #[serde(rename = "expiry")]
    expiry: String,
    #[serde(rename = "isHard", default)]
    is_hard: bool,
    #[serde(rename = "isStorm", default)]
    is_storm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fissure {
    pub tier: String,
    pub node: String,
    pub mission_type: String,
    pub enemy: String,
    pub expires_at: String,
    pub is_hard: bool,
    pub is_storm: bool,
}

fn fetch_fissures(client: &ureq::Agent) -> Result<Vec<Fissure>, String> {
    let resp = client
        .get("https://api.warframestat.us/pc/fissures")
        .call()
        .map_err(|e| format!("fissures: {e}"))?;
    let raw: Vec<FissureRaw> = resp
        .into_body()
        .read_json()
        .map_err(|e| format!("fissures parse: {e}"))?;
    Ok(raw
        .into_iter()
        .map(|f| Fissure {
            tier: f.tier,
            node: f.node,
            mission_type: f.mission_type,
            enemy: f.enemy,
            expires_at: f.expiry,
            is_hard: f.is_hard,
            is_storm: f.is_storm,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct AlertRaw {
    #[serde(rename = "mission", default)]
    mission: Option<AlertMissionRaw>,
    #[serde(rename = "expiry", default)]
    expiry: String,
}

#[derive(Debug, Deserialize)]
struct AlertMissionRaw {
    #[serde(rename = "node", default)]
    node: String,
    #[serde(rename = "type", default)]
    mission_type: String,
    #[serde(rename = "faction", default)]
    faction: String,
    #[serde(rename = "reward", default)]
    reward: Option<AlertRewardRaw>,
}

#[derive(Debug, Deserialize)]
struct AlertRewardRaw {
    #[serde(rename = "countedItems", default)]
    counted_items: Vec<AlertItemRaw>,
    #[serde(rename = "credits", default)]
    credits: i64,
}

#[derive(Debug, Deserialize)]
struct AlertItemRaw {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    item_type: String,
    #[serde(rename = "key")]
    key: String,
    #[serde(rename = "count")]
    count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub node: String,
    pub mission_type: String,
    pub faction: String,
    pub reward_text: String,
    pub expires_at: String,
}

fn fetch_alerts(client: &ureq::Agent) -> Result<Vec<Alert>, String> {
    let resp = client
        .get("https://api.warframestat.us/pc/alerts")
        .call()
        .map_err(|e| format!("alerts: {e}"))?;
    let raw: Vec<AlertRaw> = resp
        .into_body()
        .read_json()
        .map_err(|e| format!("alerts parse: {e}"))?;
    Ok(raw
        .into_iter()
        .filter_map(|a| {
            let m = a.mission?;
            Some(Alert {
                node: m.node,
                mission_type: m.mission_type,
                faction: m.faction,
                reward_text: m.reward.as_ref().map(format_reward).unwrap_or_else(|| "—".into()),
                expires_at: a.expiry,
            })
        })
        .collect())
}

fn format_reward(r: &AlertRewardRaw) -> String {
    let mut parts = Vec::new();
    for item in &r.counted_items {
        parts.push(format!("{}x {}", item.count, item.key));
    }
    if r.credits > 0 {
        parts.push(format!("{} créditos", r.credits));
    }
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join(", ")
    }
}

#[derive(Debug, Deserialize)]
struct SortieRaw {
    #[serde(rename = "variants", default)]
    variants: Vec<SortieVariantRaw>,
    #[serde(rename = "boss", default)]
    boss: String,
    #[serde(rename = "faction", default)]
    faction: String,
    #[serde(rename = "expiry", default)]
    expiry: String,
    #[serde(rename = "id", default)]
    _id: String,
    #[serde(rename = "activation", default)]
    _activation: String,
    #[serde(rename = "rewardPool", default)]
    _reward_pool: String,
    #[serde(rename = "factionKey", default)]
    _faction_key: String,
}

#[derive(Debug, Deserialize)]
struct SortieVariantRaw {
    #[serde(rename = "missionType")]
    mission_type: String,
    #[serde(rename = "modifier")]
    modifier: String,
    #[serde(rename = "node")]
    node: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sortie {
    pub missions: Vec<SortieMission>,
    pub boss: String,
    pub faction: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortieMission {
    pub mission_type: String,
    pub modifier: String,
    pub node: String,
}

fn fetch_sortie(client: &ureq::Agent) -> Result<Option<Sortie>, String> {
    let resp = client
        .get("https://api.warframestat.us/pc/sortie")
        .call()
        .map_err(|e| format!("sortie: {e}"))?;
    let raw: SortieRaw = resp
        .into_body()
        .read_json()
        .map_err(|e| format!("sortie parse: {e}"))?;
    if raw.variants.is_empty() {
        return Ok(None);
    }
    Ok(Some(Sortie {
        missions: raw
            .variants
            .into_iter()
            .map(|v| SortieMission {
                mission_type: v.mission_type,
                modifier: v.modifier,
                node: v.node,
            })
            .collect(),
        boss: raw.boss,
        faction: raw.faction,
        expires_at: raw.expiry,
    }))
}

#[derive(Debug, Deserialize)]
struct InvasionRaw {
    #[serde(rename = "node", default)]
    node: String,
    #[serde(rename = "desc", default)]
    desc: String,
    #[serde(rename = "attacker", default)]
    attacker: Option<InvasionFactionRaw>,
    #[serde(rename = "defender", default)]
    defender: Option<InvasionFactionRaw>,
    #[serde(rename = "count", default)]
    count: i64,
    #[serde(rename = "requiredRuns", default)]
    required_runs: i64,
    #[serde(rename = "completion", default)]
    completion: f64,
    #[serde(rename = "completed", default)]
    completed: bool,
}

#[derive(Debug, Deserialize)]
struct InvasionFactionRaw {
    #[serde(rename = "faction")]
    faction: String,
    #[serde(rename = "reward", default)]
    reward: Option<InvasionRewardRaw>,
}

#[derive(Debug, Deserialize)]
struct InvasionRewardRaw {
    #[serde(rename = "countedItems", default)]
    counted_items: Vec<AlertItemRaw>,
    #[serde(rename = "credits", default)]
    credits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invasion {
    pub node: String,
    pub desc: String,
    pub attacker_faction: String,
    pub attacker_reward: String,
    pub defender_faction: String,
    pub defender_reward: String,
    pub count: i64,
    pub required_runs: i64,
    pub completion: f64,
    pub completed: bool,
}

fn fetch_invasions(client: &ureq::Agent) -> Result<Vec<Invasion>, String> {
    let resp = client
        .get("https://api.warframestat.us/pc/invasions")
        .call()
        .map_err(|e| format!("invasions: {e}"))?;
    let raw: Vec<InvasionRaw> = resp
        .into_body()
        .read_json()
        .map_err(|e| format!("invasions parse: {e}"))?;
    Ok(raw
        .into_iter()
        .filter_map(|i| {
            let atk = i.attacker?;
            let def = i.defender?;
            Some(Invasion {
                node: i.node,
                desc: i.desc,
                attacker_faction: atk.faction,
                attacker_reward: atk
                    .reward
                    .as_ref()
                    .map(format_invasion_reward)
                    .unwrap_or_else(|| "—".into()),
                defender_faction: def.faction,
                defender_reward: def
                    .reward
                    .as_ref()
                    .map(format_invasion_reward)
                    .unwrap_or_else(|| "—".into()),
                count: i.count,
                required_runs: i.required_runs,
                completion: i.completion,
                completed: i.completed,
            })
        })
        .collect())
}

fn format_invasion_reward(r: &InvasionRewardRaw) -> String {
    let mut parts = Vec::new();
    for item in &r.counted_items {
        parts.push(format!("{}x {}", item.count, item.key));
    }
    if r.credits > 0 {
        parts.push(format!("{} créditos", r.credits));
    }
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join(", ")
    }
}

/// Formateo legible para el panel del TUI.
impl WorldState {
    pub fn format_fissures(&self, filter: Option<&str>) -> String {
        let mut out = String::new();
        let filtered: Vec<_> = self
            .fissures
            .iter()
            .filter(|f| filter.map_or(true, |ft| f.tier.to_lowercase() == ft.to_lowercase()))
            .collect();
        if filtered.is_empty() {
            return " (ninguna activa)".into();
        }
        for f in filtered {
            let mut tags = Vec::new();
            if f.is_storm {
                tags.push("STORM");
            }
            if f.is_hard {
                tags.push("STEEL PATH");
            }
            let tag = if tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", tags.join(" "))
            };
            let remaining = Self::format_remaining(&f.expires_at);
            out.push_str(&format!(
                "\n  - {} · {} · {}{}",
                f.tier, f.node, f.mission_type, tag
            ));
            if !remaining.is_empty() {
                out.push_str(&format!(" · {}", remaining));
            }
        }
        out
    }

    pub fn format_alerts(&self) -> String {
        if self.alerts.is_empty() {
            return " (ninguna activa)".into();
        }
        let mut out = String::new();
        for a in &self.alerts {
            let remaining = Self::format_remaining(&a.expires_at);
            out.push_str(&format!(
                "\n  - {} · {} · {} ({})",
                a.mission_type, a.node, a.faction, a.reward_text
            ));
            if !remaining.is_empty() {
                out.push_str(&format!(" · {}", remaining));
            }
        }
        out
    }

    pub fn format_sortie(&self) -> String {
        match &self.sortie {
            Some(s) => {
                let mut out = String::new();
                for (i, m) in s.missions.iter().enumerate() {
                    out.push_str(&format!(
                        "\n  {}. {} · {} ({})",
                        i + 1,
                        m.mission_type,
                        m.node,
                        m.modifier
                    ));
                }
                out.push_str(&format!("\n  Boss: {} ({})", s.boss, s.faction));
                let remaining = Self::format_remaining(&s.expires_at);
                if !remaining.is_empty() {
                    out.push_str(&format!(" · {}", remaining));
                }
                out
            }
            None => " (sin sortie activa)".into(),
        }
    }

    pub fn format_invasions(&self) -> String {
        if self.invasions.is_empty() {
            return " (ninguna activa)".into();
        }
        let mut out = String::new();
        for i in &self.invasions {
            let pct = (i.completion * 100.0).round() as i64;
            let status = if i.completed {
                "✓ COMPLETADA"
            } else {
                &format!("{}%", pct)
            };
            out.push_str(&format!(
                "\n  - {} · {} ({}) vs {} ({}) · {}",
                i.desc,
                i.attacker_faction,
                i.attacker_reward,
                i.defender_faction,
                i.defender_reward,
                status
            ));
        }
        out
    }

    pub fn format_remaining(expires_at: &str) -> String {
        match chrono::DateTime::parse_from_rfc3339(expires_at) {
            Ok(dt) => {
                let now = chrono::Utc::now();
                let dt_utc = dt.with_timezone(&chrono::Utc);
                let dur = dt_utc.signed_duration_since(now);
                if dur.num_seconds() <= 0 {
                    "EXPIRADO".into()
                } else if dur.num_hours() > 0 {
                    format!("{}h {}m restantes", dur.num_hours(), dur.num_minutes() % 60)
                } else {
                    format!("{}m restantes", dur.num_minutes())
                }
            }
            Err(_) => String::new(),
        }
    }
}
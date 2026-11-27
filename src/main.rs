//! Cephalon Mate — punto de entrada.
//!
//! TUI de chat en Rust + Ollama local. La sincronización del worldstate y la
//! base de datos de items (WFCD) corren en threads de fondo: la UI nunca se
//! bloquea esperando red.

mod audio;
mod brain;
mod config;
mod logo;
mod qa;
mod ui;

use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use brain::{
    items::{count_items, init_db, set_items_db},
    spawn_fetcher,
    tools::set_worldstate_cache,
    ChatMessage,
    OllamaClient,
    StreamMsg,
    WorldState,
};
use crate::audio::{init_audio, AudioEvent, AudioPlayer, NpcMessageKind, Personality};
use ui::{ChatEntry, UiState};

/// Frecuencia de redibujado/bucle de eventos (no quema CPU en reposo).
const TICK: Duration = Duration::from_millis(50);

/// Espera cuando no hay nada pendiente (evita el 100% de CPU).
const IDLE_SLEEP: Duration = Duration::from_millis(200);

/// Rondas máximas de tool-calling por mensaje (red de seguridad anti-bucle).
const MAX_TOOL_ROUNDS: u32 = 4;

/// Intentos de sincronización de WFCD antes de rendirse.
const WFCD_MAX_ATTEMPTS: u32 = 5;

/// Back-off base entre reintentos de WFCD (ms).
const WFCD_BACKOFF_BASE_MS: u32 = 250;

type Term = Terminal<CrosstermBackend<io::Stdout>>;

/// Mensajes que envía el thread de sincronización de WFCD.
#[derive(Debug, Clone)]
pub enum WfcdMsg {
    /// El thread arrancó.
    Started,
    /// Progreso de la descarga/escritura (0-100).
    Progress { percent: u8 },
    /// Terminó bien.
    Done { success: bool },
    /// Terminó con error tras agotar los reintentos.
    Error { msg: String },
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("cehalon-mate {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "--smoke") {
        return run_smoke();
    }
    if args.iter().any(|a| a == "--qa") {
        return run_qa_cli(args.iter().any(|a| a == "--sync"));
    }
    if let Some(pos) = args.iter().position(|a| a == "--tool") {
        let name = args.get(pos + 1).cloned().unwrap_or_default();
        let raw = args.get(pos + 2).cloned().unwrap_or_else(|| "{}".to_string());
        return run_tool_cli(&name, &raw);
    }
    if let Some(pos) = args.iter().position(|a| a == "--ask") {
        let question = args.get(pos + 1).cloned().unwrap_or_default();
        if question.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "falta la pregunta (ej: --ask \"¿Dónde dropea Serration?\")",
            ));
        }
        // `--model <nombre>` sobrescribe el modelo de Ollama (QA E2E).
        let model_override = args
            .iter()
            .position(|a| a == "--model")
            .and_then(|p| args.get(p + 1))
            .cloned();
        return run_ask_cli(&question, model_override.as_deref());
    }

    let cfg = config::Config::load(Path::new("config.toml")).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("no se pudo cargar/crear config.toml: {e}"),
        )
    })?;

    let ollama = OllamaClient::new(&cfg.ollama_host);
    let ollama_ok = ollama.health().is_ok();

    // Modelo de chat: la personalidad crea `cehalon-<nombre>`; si no existe,
    // cae al modelo base del config.
    let chat_model = if ollama
        .list_models()
        .map(|ms| ms.iter().any(|m| m.starts_with(&format!("cehalon-{}:", cfg.personality))))
        .unwrap_or(false)
    {
        format!("cehalon-{}", cfg.personality)
    } else {
        cfg.model.clone()
    };

    // BD de items: la sincronización WFCD va en background (no bloquea el
    // arranque de la UI). Solo es "urgente" si la BD está vacía.
    let items_db = init_db().map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("inicializar BD de items: {e}"),
        )
    })?;
    let count = count_items(&items_db).unwrap_or(0);
    set_items_db(items_db);
    let needs_sync = count == 0;
    if needs_sync {
        println!("[items] BD vacía: sincronizando WFCD en background…");
    } else {
        println!("[items] BD cargada: {count} items");
    }

    let state = UiState::new(
        cfg.personality.clone(),
        cfg.model.clone(),
        cfg.ollama_host.clone(),
        ollama_ok,
    );

    let result = run_tui(
        state,
        &ollama,
        &chat_model,
        cfg.worldstate_refresh_secs,
        needs_sync,
    );

    // Restaurar SIEMPRE la terminal (aunque el bucle falle).
    let mut stdout = io::stdout();
    let _ = disable_raw_mode();
    let _ = execute!(stdout, LeaveAlternateScreen);

    result
}

fn print_help() {
    println!("cehalon-mate {} — asistente de Warframe con IA local", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Uso: cehalon-mate [opciones]");
    println!();
    println!("  --version    Muestra la versión y sale");
    println!("  --smoke      Chequeo de arranque sin TUI (para CI)");
    println!("  --qa         QA de datos WFCD (warframes, mods, drops)");
    println!("  --qa --sync  QA de datos + sincroniza WFCD antes de medir");
    println!("  --tool <nombre> [json]  Ejecuta una tool y muestra su salida");
    println!("  --ask <pregunta>         Pregunta al modelo (headless, sin TUI)");
    println!("  --model <nombre>         Modelo de Ollama a usar con --ask");
    println!("  --help       Muestra esta ayuda");
}

/// Pregunta al modelo sin TUI y muestra la respuesta.
///
/// Usa exactamente el mismo flujo que la app (prefetch por intención +
/// loop de tools), así que sirve como puerta de QA end-to-end.
fn run_ask_cli(question: &str, model_override: Option<&str>) -> io::Result<()> {
    let cfg = config::Config::load(Path::new("config.toml")).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("no se pudo cargar/crear config.toml: {e}"),
        )
    })?;

    let ollama = OllamaClient::new(&cfg.ollama_host);
    if ollama.health().is_err() {
        eprintln!("[ask] Ollama no responde en {}", cfg.ollama_host);
    }
    let chat_model = match model_override {
        Some(m) => m.to_string(),
        None if ollama
            .list_models()
            .map(|ms| ms.iter().any(|m| m.starts_with(&format!("cehalon-{}:", cfg.personality))))
            .unwrap_or(false) =>
        {
            format!("cehalon-{}", cfg.personality)
        }
        None => cfg.model.clone(),
    };

    let db = init_db().map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("inicializar BD: {e}"))
    })?;
    set_items_db(db);

    let mut state = UiState::new(
        cfg.personality.clone(),
        cfg.model.clone(),
        cfg.ollama_host.clone(),
        ollama.health().is_ok(),
    );

    let rx = send_chat(
        &mut state,
        &ollama,
        &chat_model,
        question.to_string(),
        None,
    );

    let mut answer = String::new();
    for msg in rx {
        match msg {
            brain::StreamMsg::Token(t) => {
                print!("{t}");
                answer.push_str(&t);
            }
            brain::StreamMsg::Tool(label) => println!("\n[tool] {label}"),
            brain::StreamMsg::Done => {
                println!();
                break;
            }
            brain::StreamMsg::Error(e) => {
                eprintln!("\n[error] {e}");
                return Err(io::Error::new(io::ErrorKind::Other, e));
            }
        }
    }
    if answer.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "el modelo no devolvió respuesta",
        ));
    }
    Ok(())
}

/// Ejecuta una tool desde la línea de comandos e imprime su salida.
///
/// Es la puerta de QA/depuración: permite comprobar qué ve realmente el
/// modelo (`--tool donde_encontrar '{"nombre":"Serration"}'`).
fn run_tool_cli(name: &str, raw: &str) -> io::Result<()> {
    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "falta el nombre de la tool (ej: --tool buscar_item '{\"nombre\":\"Soma\"}')",
        ));
    }
    let args: serde_json::Value = parse_tool_args(raw);

    // La BD de items es necesaria para casi todas las tools.
    let db = init_db().map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("inicializar BD: {e}"))
    })?;
    set_items_db(db);

    let out = brain::tools::execute(name, &args);
    println!("{out}");
    Ok(())
}

/// Parsea los argumentos JSON de una tool.
///
/// Tolera el escapado de shell: PowerShell y cmd pasan `{\"nombre\":\"Soma\"}`
/// con backslashes literales, y otros shells pueden quitar las comillas.
/// Se prueban varias formas antes de rendirse.
fn parse_tool_args(raw: &str) -> serde_json::Value {
    let trimmed = raw.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v;
    }
    // Backslashes de escapado de shell: {\"nombre\":\"Soma\"}
    let unescaped = trimmed.replace("\\\"", "\"");
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&unescaped) {
        return v;
    }
    // Comillas dobles perdidas: {nombre:Soma}
    let loose = unescaped.replace('"', "'");
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&loose) {
        return v;
    }
    if let Some(v) = parse_loose_pairs(&loose) {
        return v;
    }
    serde_json::json!({ "_error": format!("JSON inválido: {trimmed}") })
}

/// Parsea pares `clave:valor` sin comillas: `{nombre:Soma,tipo:mod}`.
///
/// Último recurso para shells que se comen las comillas dobles.
fn parse_loose_pairs(raw: &str) -> Option<serde_json::Value> {
    let inner = raw.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut map = serde_json::Map::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) = part.split_once(':')?;
        let key = k.trim().trim_matches('\'').trim_matches('"');
        let val = v.trim().trim_matches('\'').trim_matches('"');
        if key.is_empty() {
            return None;
        }
        // Números como valor → número, no string.
        let value = match val.parse::<i64>() {
            Ok(n) => serde_json::Value::from(n),
            Err(_) => serde_json::Value::String(val.to_string()),
        };
        map.insert(key.to_string(), value);
    }
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// QA de datos: verifica warframes, mods y fuentes de drop contra la BD local.
///
/// Sale con código 1 si algún check falla (puerta usable en CI).
fn run_qa_cli(sync_first: bool) -> io::Result<()> {
    if sync_first {
        println!("[qa] sincronizando WFCD…");
        let db = init_db().map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("inicializar BD: {e}"))
        })?;
        match qa::sync_for_qa(&db) {
            Ok(n) => println!("[qa] sync OK: {n} items"),
            Err(e) => eprintln!("[qa] aviso: sync falló ({e}); se continúa con la BD actual"),
        }
    }

    let (results, fails) = qa::run_qa(sync_first);
    qa::print_report(&results, fails);

    if fails > 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("QA falló: {fails} checks en rojo"),
        ));
    }
    Ok(())
}

/// Chequeo de arranque sin TUI: carga config, abre la BD, renderiza la UI en
/// un backend de prueba y simula una actualización de worldstate.
/// Es lo que usa el smoke test de CI.
fn run_smoke() -> io::Result<()> {
    let cfg = config::Config::load(Path::new("config.toml")).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("no se pudo cargar/crear config.toml: {e}"),
        )
    })?;

    let db = init_db().map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("inicializar BD de items: {e}"),
        )
    })?;
    let count = count_items(&db).unwrap_or(0);
    set_items_db(db);

    let mut state = UiState::new(
        cfg.personality.clone(),
        cfg.model.clone(),
        cfg.ollama_host.clone(),
        true,
    );

    let screen = render_to_string(&mut state, 120, 40);
    if !screen.contains("CEPHALON MATE") || !screen.contains("CHAT") {
        return Err(io::Error::other(
            "el render del TUI no trae los paneles esperados",
        ));
    }

    // Camino de actualización de worldstate (sin red: estado simulado).
    let ws = WorldState::default();
    set_worldstate_cache(ws.clone());
    state.worldstate = Some(ws);
    let screen = render_to_string(&mut state, 120, 40);
    if !screen.contains("WORLDSTATE") {
        return Err(io::Error::other("el panel de worldstate no se renderizó"));
    }

    println!("personality: {}", cfg.personality);
    println!("model: {}", cfg.model);
    println!("ollama: {}", cfg.ollama_host);
    println!("items en BD: {count}");
    println!("✅ Worldstate actualizado");
    Ok(())
}

/// Renderiza la UI a un string (backend de prueba).
fn render_to_string(state: &mut UiState, w: u16, h: u16) -> String {
    use ratatui::backend::TestBackend;

    let mut term = Terminal::new(TestBackend::new(w, h)).expect("TestBackend");
    term.draw(|f| ui::render(state, f)).expect("draw");
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn run_tui(
    state: UiState,
    ollama: &OllamaClient,
    chat_model: &str,
    refresh_secs: u64,
    needs_sync: bool,
) -> io::Result<()> {
    // Inicializar audio player (P13).
    let mut audio_player = init_audio().map_err(|e| {
        eprintln!("[audio] No se pudo inicializar: {e}");
        io::Error::new(
            io::ErrorKind::Other,
            format!("Audio player initialization failed: {e}"),
        )
    })?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    // Spawn worldstate fetcher (P08).
    let ws_rx = spawn_fetcher(refresh_secs);

    // Canal de la sincronización WFCD en background (P09).
    let (wfcd_tx, wfcd_rx) = mpsc::channel::<WfcdMsg>();
    if needs_sync {
        spawn_wfcd_sync(wfcd_tx);
    }

    let res = event_loop(
        &mut terminal,
        state,
        ollama,
        chat_model,
        ws_rx,
        wfcd_rx,
        audio_player.clone(),
    );

    // Apagar audio al salir.
    audio_player.shutdown();

    let _ = terminal.show_cursor();
    res
}

/// Lanza la sincronización de la BD de items (WFCD) en un thread aparte.
///
/// Reintenta con back-off exponencial (250ms, 500ms, 1s, 2s…) y reporta el
/// avance por el canal para que la UI muestre el progreso sin bloquear.
pub fn spawn_wfcd_sync(tx: Sender<WfcdMsg>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let _ = tx.send(WfcdMsg::Started);
        let mut last_err = String::new();

        for attempt in 1..=WFCD_MAX_ATTEMPTS {
            let mut progress = |percent: u8| {
                let _ = tx.send(WfcdMsg::Progress { percent });
            };

            match brain::items::sync_wfcd_global(&mut progress) {
                Ok(_) => {
                    let _ = tx.send(WfcdMsg::Done { success: true });
                    return;
                }
                Err(e) => {
                    last_err = e;
                    eprintln!("[wfcd] intento {attempt}/{WFCD_MAX_ATTEMPTS} falló: {last_err}");
                    if attempt < WFCD_MAX_ATTEMPTS {
                        let backoff = Duration::from_millis(
                            (WFCD_BACKOFF_BASE_MS * 2u32.pow(attempt - 1)) as u64,
                        );
                        thread::sleep(backoff);
                    }
                }
            }
        }

        let _ = tx.send(WfcdMsg::Error { msg: last_err });
    })
}

fn event_loop(
    terminal: &mut Term,
    mut state: UiState,
    ollama: &OllamaClient,
    chat_model: &str,
    ws_rx: Receiver<WorldState>,
    wfcd_rx: Receiver<WfcdMsg>,
    audio_player: AudioPlayer,
) -> io::Result<()> {
    // Canal del stream activo (None = no hay respuesta en curso).
    let mut stream_rx: Option<Receiver<StreamMsg>> = None;

    // Canal del fetch puntual de worldstate (comando `/worldstate`).
    let (ws_manual_tx, ws_manual_rx) = mpsc::channel::<Result<WorldState, String>>();

    // Canal del sync manual de items (comando `/sync`).
    let (sync_tx, sync_rx) = mpsc::channel::<WfcdMsg>();

    loop {
        let mut busy = false;

        // 0) Poll worldstate fetcher (no bloquea).
        while let Ok(ws) = ws_rx.try_recv() {
            set_worldstate_cache(ws.clone());
            let critical = has_critical_event(&ws);
            state.worldstate = Some(ws);
            busy = true;
            // Audio: worldstate actualizado
            audio_player.play(AudioEvent::WorldstateUpdated {
                personality: state.personality_enum(),
            });
            if critical {
                audio_player.play(AudioEvent::CriticalAlert {
                    personality: state.personality_enum(),
                });
            }
        }

        // 1) Poll del sync WFCD en background.
        while let Ok(msg) = wfcd_rx.try_recv() {
            busy = true;
            apply_wfcd_msg(&mut state, &msg, &audio_player);
        }
        while let Ok(msg) = sync_rx.try_recv() {
            busy = true;
            apply_wfcd_msg(&mut state, &msg, &audio_player);
        }

        // 2) Poll del fetch manual de worldstate.
        while let Ok(result) = ws_manual_rx.try_recv() {
            busy = true;
            match result {
                Ok(ws) => {
                    set_worldstate_cache(ws.clone());
                    let critical = has_critical_event(&ws);
                    let has_alerts = !ws.alerts.is_empty();
                    state.worldstate = Some(ws);
                    state.messages.push(ChatEntry::cephalon(
                        "✅ Worldstate actualizado: fissuras, alertas, sortie e invasiones al día.",
                    ));
                    audio_player.play(AudioEvent::WorldstateUpdated {
                        personality: state.personality_enum(),
                    });
                    if critical {
                        audio_player.play(AudioEvent::CriticalAlert {
                            personality: state.personality_enum(),
                        });
                    } else if has_alerts {
                        // Hay alertas activas: el NPC avisa con su alarma.
                        audio_player.play(AudioEvent::NpcMessage {
                            npc: state.personality_enum().npc(),
                            kind: NpcMessageKind::Alert,
                        });
                    }
                }
                Err(e) => {
                    state
                        .messages
                        .push(ChatEntry::error(format!("❌ No se pudo actualizar el worldstate: {e}")));
                    audio_player.play(AudioEvent::Error);
                }
            }
        }

        // 3) Drenar tokens pendientes del stream (no bloquea el input).
        if let Some(rx) = &stream_rx {
            loop {
                match rx.try_recv() {
                    Ok(StreamMsg::Token(t)) => {
                        state.pending.push_str(&t);
                        state.thinking = false;
                        busy = true;
                    }
                    Ok(StreamMsg::Tool(label)) => {
                        state.tools_used.push(label);
                        state.thinking = false;
                        busy = true;
                        // Audio: el NPC avisa que está consultando algo
                        audio_player.play(AudioEvent::NpcMessage {
                            npc: state.personality_enum().npc(),
                            kind: NpcMessageKind::WaitingMessage,
                        });
                        // Audio: tool ejecutada
                        audio_player.play(AudioEvent::ToolExecuted { success: true });
                    }
                    Ok(StreamMsg::Done) | Err(mpsc::TryRecvError::Disconnected) => {
                        let text = std::mem::take(&mut state.pending);
                        if !text.trim().is_empty() {
                            state.messages.push(ChatEntry::cephalon(text));
                            // Audio: llegó la respuesta del NPC
                            audio_player.play(AudioEvent::NpcMessage {
                                npc: state.personality_enum().npc(),
                                kind: NpcMessageKind::Arrived,
                            });
                        }
                        state.thinking = false;
                        stream_rx = None;
                        busy = true;
                        break;
                    }
                    Ok(StreamMsg::Error(e)) => {
                        state.pending.clear();
                        state.messages.push(ChatEntry::error(e));
                        state.thinking = false;
                        stream_rx = None;
                        busy = true;
                        // Audio: error
                        audio_player.play(AudioEvent::Error);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }

        terminal.draw(|f| ui::render(&mut state, f))?;

        if event::poll(TICK)? {
            busy = true;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Ctrl+C siempre sale.
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
                    {
                        return Ok(());
                    }
                    match key.code {
                        KeyCode::Char(c) => {
                            state.input.push(c);
                            state.history_idx = None;
                        }
                        KeyCode::Backspace => {
                            state.input.pop();
                            state.history_idx = None;
                        }
                        KeyCode::Tab => {
                            handle_tab(&mut state, &audio_player);
                        }
                        KeyCode::Enter => {
                            // No enviar otro mensaje a la vez (el input sigue vivo).
                            if stream_rx.is_some() {
                                continue;
                            }
                            let msg = state.input.trim().to_string();
                            if msg.is_empty() {
                                continue;
                            }
                            state.input.clear();
                            state.history.push(msg.clone());
                            state.history_idx = None;

                            // Procesar comandos que empiezan con /
                            if msg.starts_with('/') {
                                handle_command(
                                    &mut state,
                                    &msg,
                                    ollama,
                                    chat_model,
                                    &audio_player,
                                    &ws_manual_tx,
                                    &sync_tx,
                                );
                                continue;
                            }

                            stream_rx = Some(send_chat(
                                &mut state,
                                ollama,
                                chat_model,
                                msg,
                                Some(&audio_player),
                            ));
                        }
                        KeyCode::Up => history_up(&mut state),
                        KeyCode::Down => history_down(&mut state),
                        _ => {}
                    }
                }
                // Resize: el siguiente `draw` re-splittea el layout solo.
                _ => {}
            }
        }

        // Sin nada pendiente: dormir para no quemar CPU.
        if !busy {
            thread::sleep(IDLE_SLEEP);
        }
    }
}

/// ¿Hay un evento que merezca sonido de alerta? (fissura Steel Path).
fn has_critical_event(ws: &WorldState) -> bool {
    ws.fissures.iter().any(|f| f.is_hard)
}

/// Traduce un mensaje del thread de WFCD a la UI.
fn apply_wfcd_msg(state: &mut UiState, msg: &WfcdMsg, audio_player: &AudioPlayer) {
    match msg {
        WfcdMsg::Started => {
            state.messages.push(ChatEntry::cephalon(
                "📦 Sincronizando base de datos de items con WFCD…",
            ));
        }
        WfcdMsg::Progress { percent } => {
            // El progreso se ve en la barra del input; no ensuciamos el chat.
            state.tools_used = vec![format!("wfcd {percent}%")];
        }
        WfcdMsg::Done { success: true } => {
            state.tools_used.clear();
            state.messages.push(ChatEntry::cephalon(
                "✅ Base de datos de items (WFCD) actualizada.",
            ));
            audio_player.play(AudioEvent::ToolExecuted { success: true });
        }
        WfcdMsg::Done { success: false } => {
            state.tools_used.clear();
            state.messages.push(ChatEntry::error(
                "❌ La sincronización WFCD no se completó.",
            ));
            audio_player.play(AudioEvent::Error);
        }
        WfcdMsg::Error { msg } => {
            state.tools_used.clear();
            state.messages.push(ChatEntry::error(format!(
                "❌ Error sincronizando WFCD: {msg}"
            )));
            audio_player.play(AudioEvent::Error);
        }
    }
}

/// Flecha arriba: recorrer el historial de envíos (más viejo → más nuevo).
fn history_up(state: &mut UiState) {
    if state.history.is_empty() {
        return;
    }
    let idx = match state.history_idx {
        None => state.history.len() - 1,
        Some(0) => 0,
        Some(i) => i - 1,
    };
    state.history_idx = Some(idx);
    state.input = state.history[idx].clone();
}

/// Flecha abajo: avanzar en el historial; al final, volver a vacío.
fn history_down(state: &mut UiState) {
    match state.history_idx {
        Some(i) if i + 1 < state.history.len() => {
            state.history_idx = Some(i + 1);
            state.input = state.history[i + 1].clone();
        }
        Some(_) => {
            state.history_idx = None;
            state.input.clear();
        }
        None => {}
    }
}

/// Comandos disponibles para autocompletado con Tab.
const COMMANDS: &[&str] = &[
    "/personality",
    "/model",
    "/status",
    "/worldstate",
    "/sync",
    "/clear",
    "/quit",
    "/help",
];

#[doc(hidden)]
thread_local! {
    static TAB_LAST_IDX: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Maneja comandos que empiezan con '/'.
#[allow(clippy::too_many_arguments)]
fn handle_command(
    state: &mut UiState,
    cmd: &str,
    ollama: &OllamaClient,
    chat_model: &str,
    audio_player: &AudioPlayer,
    ws_manual_tx: &Sender<Result<WorldState, String>>,
    sync_tx: &Sender<WfcdMsg>,
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }
    let cmd_name = parts[0].to_lowercase();
    let args = &parts[1..];

    match cmd_name.as_str() {
        "/help" | "/?" => {
            state.messages.push(ChatEntry::cephalon(
                "📋 **Comandos disponibles:**\n\
                 \n\
                 /personality <ordis|suda|simaris>  — Cambiar personalidad del Cephalon\n\
                 /model <nombre>                     — Cambiar modelo de Ollama (ej. qwen2.5:3b)\n\
                 /status                             — Mostrar estado del sistema\n\
                 /worldstate                         — Forzar actualización del worldstate\n\
                 /sync                               — Re-sincronizar la BD de items con WFCD\n\
                 /clear                              — Limpiar el historial de chat\n\
                 /quit                               — Salir de la aplicación\n\
                 /help                               — Mostrar esta ayuda\n\
                 \n\
                 **Teclas:**\n\
                 Tab — Autocompletar comandos\n\
                 ↑/↓ — Historial de mensajes\n\
                 Ctrl+C — Salir"
            ));
        }
        "/quit" | "/exit" => {
            state.messages.push(ChatEntry::cephalon("👋 ¡Hasta la próxima, Tenno!"));
            std::thread::sleep(Duration::from_millis(500));
            std::process::exit(0);
        }
        "/clear" => {
            state.messages.clear();
            state
                .messages
                .push(ChatEntry::cephalon("🧹 Chat limpiado. ¡Listo para nuevas órdenes, Tenno!"));
            audio_player.play(AudioEvent::ToolExecuted { success: true });
        }
        "/status" => {
            let models = ollama.list_models().unwrap_or_default();
            let has_personality = models
                .iter()
                .any(|m| m.starts_with(&format!("cehalon-{}:", state.personality)));
            let active_model = if has_personality {
                format!("cehalon-{} (personalidad)", state.personality)
            } else {
                state.model.clone()
            };
            let ws_age = state
                .worldstate
                .as_ref()
                .map(|ws| format!("{}s", ws.fetched_at.elapsed().as_secs()))
                .unwrap_or_else(|| "sin datos".to_string());

            state.messages.push(ChatEntry::cephalon(format!(
                "📊 **Estado del sistema:**\n\
                 \n\
                 🎭 Personalidad: **{}**\n\
                 🧠 Modelo activo: **{}**\n\
                 🧩 Modelo de chat: **{}**\n\
                 🌐 Ollama: **{}** {}\n\
                 🌍 Worldstate: **{}** (actualizado hace {})\n\
                 💬 Mensajes en chat: **{}**\n\
                 🔧 Tools disponibles: worldstate, buscar_item, consultar_precio, buscar_guia",
                state.personality.to_uppercase(),
                active_model,
                chat_model,
                ollama.host(),
                if ollama.health().is_ok() { "✅ OK" } else { "❌ OFFLINE" },
                if state.worldstate.is_some() { "activo" } else { "esperando" },
                ws_age,
                state.messages.len()
            )));
        }
        "/worldstate" => {
            state.messages.push(ChatEntry::cephalon(
                "🔄 Forzando actualización del worldstate…",
            ));
            // Fetch puntual en background: la UI sigue respondiendo.
            let tx = ws_manual_tx.clone();
            thread::spawn(move || {
                let result = brain::worldstate::fetch_now();
                let _ = tx.send(result);
            });
        }
        "/sync" => {
            state.messages.push(ChatEntry::cephalon(
                "🔄 Re-sincronizando la base de datos de items con WFCD…",
            ));
            let tx = sync_tx.clone();
            spawn_wfcd_sync(tx);
        }
        "/personality" => {
            if args.is_empty() {
                state.messages.push(ChatEntry::error(
                    "Uso: /personality <ordis|suda|simaris>\n\
                     Personalidades disponibles: ordis, suda, simaris",
                ));
                return;
            }
            let new_p = args[0].to_lowercase();
            match new_p.as_str() {
                "ordis" | "suda" | "simaris" => {
                    let model_name = format!("cehalon-{}", new_p);

                    // Verificar si el modelo existe en Ollama
                    let models = ollama.list_models().unwrap_or_default();
                    if !models.iter().any(|m| m.starts_with(&model_name)) {
                        // Modelo no existe: crearlo automáticamente desde el Modelfile
                        state.messages.push(ChatEntry::cephalon(format!(
                            "🔧 Modelo '{}' no encontrado. Creándolo desde modelfiles/{}.Modelfile...",
                            model_name, new_p
                        )));

                        let modelfile_path = format!("modelfiles/{}.Modelfile", new_p);
                        let modelfile_content = std::fs::read_to_string(&modelfile_path)
                            .map_err(|e| format!("No se pudo leer {}: {}", modelfile_path, e));

                        match modelfile_content {
                            Ok(content) => {
                                state.messages.push(ChatEntry::cephalon(
                                    "⏳ Creando modelo en Ollama... (puede tardar unos segundos)",
                                ));

                                // Llamar a Ollama API para crear el modelo
                                let create_url = format!("{}/api/create", ollama.host());
                                let body = serde_json::json!({
                                    "name": model_name,
                                    "modelfile": content
                                });

                                let client = ureq::Agent::new_with_config(
                                    ureq::Agent::config_builder()
                                        .timeout_global(Some(Duration::from_secs(120)))
                                        .build(),
                                );

                                let resp = client.post(&create_url).send_json(&body);

                                match resp {
                                    Ok(r) => {
                                        if r.status() == 200 {
                                            state.messages.push(ChatEntry::cephalon(format!(
                                                "✅ Modelo '{}' creado correctamente.",
                                                model_name
                                            )));
                                            audio_player.play(AudioEvent::ToolExecuted { success: true });
                                        } else {
                                            // Obtener status ANTES de consumir el body
                                            let status = r.status();
                                            // Leer el body como string (consumiendo la respuesta)
                                            let body = r.into_body();
                                            let mut body_reader = body.into_reader();
                                            let mut err_text = String::new();
                                            std::io::Read::read_to_string(&mut body_reader, &mut err_text).ok();
                                            state.messages.push(ChatEntry::error(format!(
                                                "❌ Error creando modelo: HTTP {} - {}",
                                                status, err_text
                                            )));
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        state.messages.push(ChatEntry::error(format!(
                                            "❌ Error conectando a Ollama: {}",
                                            e
                                        )));
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                state.messages.push(ChatEntry::error(format!(
                                    "❌ No se encontró el Modelfile: {}",
                                    e
                                )));
                                return;
                            }
                        }
                    }

                    // Actualizar personalidad y modelo activo
                    state.personality = new_p.clone();
                    state.messages.push(ChatEntry::cephalon(format!(
                        "🎭 Personalidad cambiada a **{}**.\n\
                         Modelo activo: **cehalon-{}**",
                        new_p.to_uppercase(),
                        new_p
                    )));
                    // Audio: personalidad cambiada (set de sonido del NPC)
                    audio_player.play(AudioEvent::CephalonMessage {
                        personality: Personality::from_str(&new_p),
                    });
                }
                _ => {
                    state.messages.push(ChatEntry::error(
                        "Personalidad desconocida. Opciones: ordis, suda, simaris",
                    ));
                }
            }
        }
        "/model" => {
            if args.is_empty() {
                let models = ollama.list_models().unwrap_or_default();
                state.messages.push(ChatEntry::cephalon(format!(
                    "Modelo actual: **{}**\n\nModelos disponibles:\n{}",
                    state.model,
                    models.iter().map(|m| format!("  • {}", m)).collect::<Vec<_>>().join("\n")
                )));
                return;
            }
            let new_model = args.join(" ");
            let models = ollama.list_models().unwrap_or_default();
            if !models.iter().any(|m| m == &new_model) {
                state.messages.push(ChatEntry::error(format!(
                    "❌ El modelo '{}' no está instalado en Ollama.\n\
                     Modelos disponibles:\n{}",
                    new_model,
                    models.iter().map(|m| format!("  • {}", m)).collect::<Vec<_>>().join("\n")
                )));
                return;
            }
            state.model = new_model.clone();
            state.messages.push(ChatEntry::cephalon(format!(
                "🧠 Modelo cambiado a **{}**",
                new_model
            )));
            audio_player.play(AudioEvent::ToolExecuted { success: true });
        }
        _ => {
            state.messages.push(ChatEntry::error(format!(
                "❌ Comando desconocido: '{}'\n\
                 Escribe /help para ver la lista de comandos.",
                cmd_name
            )));
            audio_player.play(AudioEvent::Error);
        }
    }
}

/// Autocompletado con Tab: cicla entre comandos que coincidan.
fn handle_tab(state: &mut UiState, audio_player: &AudioPlayer) {
    let input = state.input.trim_start_matches('/').to_string();
    if input.is_empty() {
        // Si no hay nada, ciclar por todos
        TAB_LAST_IDX.with(|cell| {
            let next = (cell.get() + 1) % COMMANDS.len();
            cell.set(next);
            state.input = format!("/{} ", COMMANDS[next]);
        });
        audio_player.play(AudioEvent::ToolExecuted { success: true });
        return;
    }
    // Buscar comandos que empiecen por lo escrito
    let matches: Vec<&str> = COMMANDS
        .iter()
        .filter(|c| c.starts_with(&format!("/{input}")))
        .copied()
        .collect();
    if matches.is_empty() {
        return;
    }
    // Encontrar el actual y avanzar al siguiente
    TAB_LAST_IDX.with(|cell| {
        let current = format!("/{input}");
        let idx = matches.iter().position(|c| *c == current).unwrap_or(0);
        let next = (idx + 1) % matches.len();
        cell.set(next);
        state.input = format!("{} ", matches[next]);
    });
    audio_player.play(AudioEvent::ToolExecuted { success: true });
}

/// Lanza el mensaje en un thread de streaming y devuelve el canal de tokens.
///
/// Loop de tools: si el modelo pide ejecutar una, se ejecuta, se devuelven los
/// resultados y se hace un segundo pase (máx. `MAX_TOOL_ROUNDS` para no
/// ciclar infinitamente).
fn send_chat(
    state: &mut UiState,
    ollama: &OllamaClient,
    chat_model: &str,
    msg: String,
    audio_player: Option<&AudioPlayer>,
) -> Receiver<StreamMsg> {
    state.messages.push(ChatEntry::you(msg.clone()));
    state.thinking = true;
    state.pending.clear();
    state.tools_used.clear();

    // Audio: nuevo mensaje del usuario → Cephalon va a responder
    // Audio: nuevo mensaje del usuario → Cephalon va a responder
    if let Some(a) = audio_player {
        a.play(AudioEvent::CephalonMessage {
            personality: state.personality_enum(),
        });
        // Audio: el NPC queda a la espera de la respuesta
        a.play(AudioEvent::NpcMessage {
            npc: state.personality_enum().npc(),
            kind: NpcMessageKind::Waiting,
        });
    }

    // Historial completo de la conversación para el modelo.
    let history: Vec<ChatMessage> = state
        .messages
        .iter()
        .filter_map(|m| match m.who {
            ui::ChatWho::You => Some(ChatMessage::user(m.text.clone())),
            ui::ChatWho::Cephalon => Some(ChatMessage::assistant(m.text.clone())),
            ui::ChatWho::Error => None,
        })
        .collect();

    let (tx, rx) = mpsc::channel();
    let host = ollama.host().to_string();
    let model = chat_model.to_string();
    let audio_for_thread = audio_player.cloned();

    thread::spawn(move || {
        let client = OllamaClient::new(host);
        let tools = brain::tools::all_tools();
        let mut messages = history;

        // Prefetch: si la pregunta es sobre datos del juego (item, mod,
        // warframe, drop), se ejecuta la tool ANTES de preguntar al modelo y
        // se le inyectan los datos verificados. Así el modelo no alucina
        // polaridades ni drops aunque no sepa llamar a las tools por su cuenta.
        let last_user_text = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let intents = prefetch_intents(&messages);
        let unknowns = prefetch_unknowns(&messages);
        let asks_game_data = brain::intent::asks_game_data(&last_user_text);

        let mut block = String::new();
        for intent in &intents {
            let output = brain::tools::execute(&intent.tool, &intent.args);
            block.push_str(&format!(
                "\n\n[DATOS VERIFICADOS — herramienta `{}`]\n{}\n\
                 [Fin de los datos verificados. Usa EXCLUSIVAMENTE esta información: \
                 cita cada fuente con su probabilidad tal como aparece, sin inventar ni \
                 atribuir la polaridad al enemigo.]",
                intent.tool, output
            ));
            // La UI muestra la tool en la línea verde (como si el modelo la
            // hubiera pedido), para que el Tenno vea de dónde salen los datos.
            let _ = tx.send(StreamMsg::Tool(intent.label.clone()));
            if let Some(a) = &audio_for_thread {
                a.play(AudioEvent::ToolExecuted { success: true });
            }
        }

        // Items que el usuario menciona y que NO están en el dataset: se avisa
        // explícitamente para que el modelo no invente drops ni recetas.
        for unknown in &unknowns {
            block.push_str(&format!(
                "\n\n[ITEM NO CATALOGADO — \"{unknown}\"]: no hay datos de WFCD para este item.",
            ));
        }

        // Guard anti-alucinación: solo si la pregunta pide datos del juego.
        if asks_game_data {
            block.push_str(
                "\n\n[REGLAS DE RESPUESTA]\n\
                 - Redacta tu respuesta en tu propio estilo, sin copiar estos bloques \
                 literalmente: resume los datos verificados (máximo 10 líneas) y cita las \
                 fuentes con su probabilidad tal como aparecen.\n\
                 - Si la pregunta es por dónde conseguir algo, da como mínimo DOS fuentes \
                 concretas: nombre exacto de la fuente + probabilidad (ej. \"Charger — 33 %\"). \
                 No digas sólo \"en misiones\".\n\
                 - No afirmes drops, polaridades, stats, precios, ingredientes ni recetas \
                 que no aparezcan en los datos verificados.\n\
                 - Para un item marcado como NO CATALOGADO, di con honestidad que ese \
                 dato no está disponible (ejemplo: \"Ordis no tiene ese dato en su \
                 memoria, Tenno\") y no lo inventes.",
            );
        }

        if !block.is_empty() {
            if let Some(last) = messages.last_mut() {
                if last.role == "user" {
                    last.content.push_str(&block);
                }
            }
        }

        for _round in 0..MAX_TOOL_ROUNDS {
            let result = client.chat_stream(&model, &messages, Some(&tools), |token| {
                let _ = tx.send(StreamMsg::Token(token.to_string()));
            });

            let calls = match result {
                Ok(calls) => calls,
                Err(e) => {
                    let _ = tx.send(StreamMsg::Error(e));
                    return;
                }
            };

            if calls.is_empty() {
                // Respuesta final sin tools: el contenido ya se transmitió.
                let _ = tx.send(StreamMsg::Done);
                return;
            }

            // Ejecutar cada tool pedida y devolver los resultados.
            messages.push(ChatMessage::assistant_tools(calls.clone()));
            for call in &calls {
                let name = call.function.name.clone();
                let label = brain::tools::call_label(&name, &call.function.arguments);
                let _ = tx.send(StreamMsg::Tool(label));

                // Audio: tool ejecutada
                if let Some(a) = &audio_for_thread {
                    a.play(AudioEvent::ToolExecuted { success: true });
                }

                let output = brain::tools::execute(&name, &call.function.arguments);
                messages.push(ChatMessage::tool_result(&name, output));
            }
        }

        // Se agotaron los rounds: avisar para no dejar el spinner colgado.
        let _ = tx.send(StreamMsg::Error(
            "demasiadas rondas de herramientas seguidas (¿bucle?)".into(),
        ));
    });

    rx
}

/// Detecta las intenciones del último mensaje del usuario.
///
/// Devuelve las tools a ejecutar si la pregunta es claramente sobre datos del
/// juego y hay items conocidos en la BD.
fn prefetch_intents(messages: &[ChatMessage]) -> Vec<brain::intent::Intent> {
    let Some(last_user) = messages.iter().rev().find(|m| m.role == "user") else {
        return Vec::new();
    };
    let Some(arc) = brain::items::get_items_db() else {
        return Vec::new();
    };
    // El lock se suelta antes de ejecutar las tools (que vuelven a tomarlo).
    let intents = {
        let conn = match arc.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        brain::intent::detect_all(&conn, &last_user.content)
    };
    intents
}

/// Detecta items mencionados por el usuario que **no** están en la BD.
///
/// Sirve para advertir al modelo de que ese item no existe en el dataset
/// (y que no debe inventar datos sobre él).
fn prefetch_unknowns(messages: &[ChatMessage]) -> Vec<String> {
    let Some(last_user) = messages.iter().rev().find(|m| m.role == "user") else {
        return Vec::new();
    };
    let Some(arc) = brain::items::get_items_db() else {
        return Vec::new();
    };
    let unknowns = {
        let conn = match arc.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        brain::intent::unknown_items(&conn, &last_user.content)
    };
    // Sólo tiene sentido avisar si la pregunta pide datos del juego.
    if brain::intent::asks_game_data(&last_user.content) {
        unknowns.into_iter().take(3).collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wfcd_backoff_is_exponential() {
        // 250ms, 500ms, 1000ms, 2000ms…
        let expected = [250u64, 500, 1000, 2000, 4000];
        for (i, ms) in expected.iter().enumerate() {
            let attempt = i as u32 + 1;
            let backoff = WFCD_BACKOFF_BASE_MS * 2u32.pow(attempt - 1);
            assert_eq!(backoff as u64, *ms);
        }
    }

    #[test]
    fn wfcd_messages_are_debuggable() {
        let msg = WfcdMsg::Progress { percent: 42 };
        assert!(format!("{msg:?}").contains("42"));
    }

    #[test]
    fn tool_args_tolerate_shell_escaping() {
        // JSON normal.
        let v = parse_tool_args(r#"{"nombre":"Soma"}"#);
        assert_eq!(v["nombre"], "Soma");

        // PowerShell/cmd escapan las comillas con backslash.
        let v = parse_tool_args(r#"{\"nombre\":\"Soma\"}"#);
        assert_eq!(v["nombre"], "Soma");

        // Shell que se come las comillas.
        let v = parse_tool_args(r#"{nombre:Soma}"#);
        assert_eq!(v["nombre"], "Soma");

        // JSON inválido de verdad → error explícito, no panic.
        let v = parse_tool_args("esto no es json");
        assert!(v.get("_error").is_some());
    }
}

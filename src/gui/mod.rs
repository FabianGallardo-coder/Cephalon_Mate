//! GUI layer using egui/eframe — immediate mode, liviano, moderno.
//! Se activa con `cargo run --release --features gui -- --gui`

pub mod assets;
pub mod theme;
pub mod widgets;

use crate::audio::{AudioEvent, AudioPlayer, NpcMessageKind, Personality};
use crate::brain::chat::{ChatMessage, StreamMsg};
use crate::brain::tools::ToolCall;
use crate::brain::client::OllamaClient;
use crate::brain::tools;
use crate::brain::worldstate::WorldState;
use crate::config::Config;
use crate::ui::ChatEntry;
use crate::brain::intent::{prefetch_intents, prefetch_unknowns, asks_game_data};
use std::sync::mpsc::{Receiver, Sender, channel};
use eframe::egui;
use egui::{Color32, TextureHandle, Vec2};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Estado compartido entre el event loop y la GUI.
#[derive(Clone)]
pub struct SharedState {
    pub personality: Arc<Mutex<Personality>>,
    pub model: Arc<Mutex<String>>,
    pub ollama_host: Arc<Mutex<String>>,
    pub ollama_ok: Arc<Mutex<bool>>,
    pub worldstate: Arc<Mutex<Option<WorldState>>>,
    pub messages: Arc<Mutex<Vec<ChatEntry>>>,
    pub thinking: Arc<Mutex<bool>>,
    pub pending: Arc<Mutex<String>>,
    pub tools_used: Arc<Mutex<Vec<String>>>,
    pub input: Arc<Mutex<String>>,
    pub history: Arc<Mutex<Vec<String>>>,
    pub history_idx: Arc<Mutex<Option<usize>>>,
    pub quit: Arc<Mutex<bool>>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            personality: Arc::new(Mutex::new(Personality::Ordis)),
            model: Arc::new(Mutex::new(String::new())),
            ollama_host: Arc::new(Mutex::new(String::new())),
            ollama_ok: Arc::new(Mutex::new(false)),
            worldstate: Arc::new(Mutex::new(None)),
            messages: Arc::new(Mutex::new(Vec::new())),
            thinking: Arc::new(Mutex::new(false)),
            pending: Arc::new(Mutex::new(String::new())),
            tools_used: Arc::new(Mutex::new(Vec::new())),
            input: Arc::new(Mutex::new(String::new())),
            history: Arc::new(Mutex::new(Vec::new())),
            history_idx: Arc::new(Mutex::new(None)),
            quit: Arc::new(Mutex::new(false)),
        }
    }
}

/// Aplicación principal de la GUI.
pub struct CephalonGui {
    state: SharedState,
    ollama: OllamaClient,
    audio_player: Option<AudioPlayer>,
    logo_texture: Option<TextureHandle>,
    sprites: assets::SpriteSheet,
    // Canales para comunicación con thread de streaming
    stream_tx: Option<Sender<StreamMsg>>,
    stream_rx: Option<Receiver<StreamMsg>>,
    // Worldstate manual fetch
    ws_manual_tx: Option<Sender<Result<WorldState, String>>>,
    ws_manual_rx: Option<Receiver<Result<WorldState, String>>>,
    // WFCD sync
    sync_tx: Option<Sender<crate::WfcdMsg>>,
    sync_rx: Option<Receiver<crate::WfcdMsg>>,
    // Worldstate background fetcher
    ws_bg_rx: Option<Receiver<WorldState>>,
    // WFCD background sync
    wfcd_bg_rx: Option<Receiver<crate::WfcdMsg>>,
    // Config
    config: Config,
    // UI state
    last_repaint: Instant,
}

impl CephalonGui {
    /// Crea una nueva instancia de la GUI.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Instalar cargadores de imagen
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Cargar config
        let config = Config::load(std::path::Path::new("config.toml")).unwrap_or_default();

        // Cliente Ollama
        let ollama = OllamaClient::new(&config.ollama_host);
        let ollama_ok = ollama.health().is_ok();

        // Audio player (opcional)
        let audio_player = crate::audio::init_audio();

        // Assets (sprites HUD, logo)
        let sprites = assets::SpriteSheet::load();

        // Logo texture
        let logo_texture = sprites.logo.as_ref().map(|img| {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            cc.egui_ctx.load_texture(
                "cephalon_logo",
                egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                egui::TextureOptions::default(),
            )
        });

        // Shared state inicial
        let state = SharedState {
            personality: Arc::new(Mutex::new(Personality::from_str(&config.personality))),
            model: Arc::new(Mutex::new(config.model.clone())),
            ollama_host: Arc::new(Mutex::new(config.ollama_host.clone())),
            ollama_ok: Arc::new(Mutex::new(ollama_ok)),
            ..Default::default()
        };

        // Canales
        let (stream_tx, stream_rx) = channel();
        let (ws_manual_tx, ws_manual_rx) = channel();
        let (sync_tx, sync_rx) = channel();

        // Background fetchers
        let ws_bg_rx = crate::brain::worldstate::spawn_fetcher(config.worldstate_refresh_secs);
        let wfcd_bg_rx = {
            let (tx, rx) = channel();
            crate::spawn_wfcd_sync(tx);
            rx
        };

        Self {
            state,
            ollama,
            audio_player: Some(audio_player),
            logo_texture,
            sprites,
            stream_tx: Some(stream_tx),
            stream_rx: Some(stream_rx),
            ws_manual_tx: Some(ws_manual_tx),
            ws_manual_rx: Some(ws_manual_rx),
            sync_tx: Some(sync_tx),
            sync_rx: Some(sync_rx),
            ws_bg_rx: Some(ws_bg_rx),
            wfcd_bg_rx: Some(wfcd_bg_rx),
            config,
            last_repaint: Instant::now(),
        }
    }

    /// Envía un mensaje al modelo (inicia streaming en thread aparte).
    fn send_message(&mut self) {
        let input = self.state.input.lock().unwrap().trim().to_string();
        if input.is_empty() {
            return;
        }

        // Agregar al historial
        self.state.messages.lock().unwrap().push(ChatEntry::you(input.clone()));
        self.state.history.lock().unwrap().push(input.clone());
        self.state.history_idx.lock().unwrap().take();
        *self.state.input.lock().unwrap() = String::new();
        *self.state.thinking.lock().unwrap() = true;
        *self.state.pending.lock().unwrap() = String::new();
        self.state.tools_used.lock().unwrap().clear();

        // Audio: mensaje del usuario
        if let Some(a) = &self.audio_player {
            a.play(AudioEvent::CephalonMessage {
                personality: *self.state.personality.lock().unwrap(),
            });
        }

        // Prefetch intents (anti-alucinación) - igual que TUI
        let messages = self.state.messages.lock().unwrap();
        let history: Vec<ChatMessage> = messages
            .iter()
            .filter_map(|m| match m.who {
                crate::ui::ChatWho::You => Some(ChatMessage::user(m.text.clone())),
                crate::ui::ChatWho::Cephalon => Some(ChatMessage::assistant(m.text.clone())),
                crate::ui::ChatWho::Error => None,
            })
            .collect();
        drop(messages);

        let last_user_text = history.iter().rev().find(|m| m.role == "user").map(|m| m.content.clone()).unwrap_or_default();
        let intents = prefetch_intents(&history);
        let unknowns = prefetch_unknowns(&history);
        let asks_game_data = asks_game_data(&last_user_text);

        // Construir bloque de datos verificados
        let mut block = String::new();
        let tools_vec = tools::all_tools();
        for intent in &intents {
            let output = tools::execute(&intent.tool, &intent.args);
            block.push_str(&format!(
                "\n\n[DATOS VERIFICADOS — herramienta `{}`]\n{}\n[Fin de los datos verificados.]",
                intent.tool, output
            ));
            // Mostrar tool en HUD
            if let Some(tx) = &self.stream_tx {
                let _ = tx.send(StreamMsg::Tool(intent.label.clone()));
            }
            if let Some(a) = &self.audio_player {
                a.play(AudioEvent::ToolExecuted { success: true });
            }
        }

        // Guard anti-alucinación
        if asks_game_data {
            block.push_str(
                "\n\n[REGLAS DE RESPUESTA]\n- Resume datos verificados (máx 10 líneas) y cita fuentes con probabilidad.\n- Si preguntas por drops: mínimo DOS fuentes con nombre exacto + %.\n- No inventes drops, polaridades, stats, precios, ingredientes ni recetas.\n- Para ITEM NO CATALOGADO: di que no tienes el dato, no lo inventes.",
            );
        }

        // Items desconocidos
        for unk in &unknowns {
            block.push_str(&format!(
                "\n\n[ITEM NO CATALOGADO: {}] — no está en la BD de WFCD. No lo inventes.",
                unk
            ));
        }

        // Inyectar bloque en último mensaje de usuario
        if !block.is_empty() {
            let mut messages = self.state.messages.lock().unwrap();
            if let Some(last) = messages.last_mut() {
                if matches!(last.who, crate::ui::ChatWho::You) {
                    last.text.push_str(&block);
                }
            }
        }

        // Preparar para streaming
        let model = self.state.model.lock().unwrap().clone();
        let tools = tools_vec;
        let host = self.ollama.host().to_string();
        let tx = self.stream_tx.clone().unwrap();
        let audio_tx = self.audio_player.clone();

        // Thread de streaming
        thread::spawn(move || {
            let client = OllamaClient::new(host);
            let mut messages = history;

            const MAX_TOOL_ROUNDS: usize = 4;
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
                    let _ = tx.send(StreamMsg::Done);
                    return;
                }

                // Ejecutar tools pedidas
                messages.push(ChatMessage::assistant_tools(calls.clone()));
                for call in &calls {
                    let output = tools::execute(&call.function.name, &call.function.arguments);
                    messages.push(ChatMessage::tool_result(&call.function.name, &output));
                    let _ = tx.send(StreamMsg::Tool(format!("{} → ok", call.function.name)));
                    if let Some(a) = &audio_tx {
                        a.play(AudioEvent::ToolExecuted { success: true });
                    }
                }
            }
            // Se agotaron rounds
            let _ = tx.send(StreamMsg::Error(String::from("Máximo de rondas de tools alcanzado")));
        });
    }

    /// Poll de canales background y actualiza shared state.
    fn poll_background(&mut self) {
        // Worldstate background
        if let Some(rx) = &self.ws_bg_rx {
            while let Ok(ws) = rx.try_recv() {
                *self.state.worldstate.lock().unwrap() = Some(ws.clone());
                // Audio worldstate
                if let Some(a) = &self.audio_player {
                    a.play(AudioEvent::WorldstateUpdated {
                        personality: *self.state.personality.lock().unwrap(),
                    });
                    // Critical alert?
                    if ws.fissures.iter().any(|f| f.is_hard) {
                        a.play(AudioEvent::CriticalAlert {
                            personality: *self.state.personality.lock().unwrap(),
                        });
                    }
                }
            }
        }

        // WFCD sync background
        if let Some(rx) = &self.wfcd_bg_rx {
            while let Ok(msg) = rx.try_recv() {
                crate::apply_wfcd_msg_gui(&mut self.state, &msg, self.audio_player.as_ref());
            }
        }

        // Worldstate manual fetch response
        if let Some(rx) = &self.ws_manual_rx {
            while let Ok(result) = rx.try_recv() {
                match result {
                    Ok(ws) => {
                        *self.state.worldstate.lock().unwrap() = Some(ws.clone());
                        self.state.messages.lock().unwrap().push(ChatEntry::cephalon(
                            String::from("✅ Worldstate actualizado manualmente."),
                        ));
                        if let Some(a) = &self.audio_player {
                            a.play(AudioEvent::ToolExecuted { success: true });
                        }
                    }
                    Err(e) => {
                        self.state.messages.lock().unwrap().push(ChatEntry::error(format!(
                            "❌ Error worldstate: {e}"
                        )));
                        if let Some(a) = &self.audio_player {
                            a.play(AudioEvent::Error);
                        }
                    }
                }
            }
        }

        // WFCD manual sync response
        if let Some(rx) = &self.sync_rx {
            while let Ok(msg) = rx.try_recv() {
                crate::apply_wfcd_msg_gui(&mut self.state, &msg, self.audio_player.as_ref());
            }
        }

        // Stream messages
        if let Some(rx) = &self.stream_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    StreamMsg::Token(t) => {
                        self.state.pending.lock().unwrap().push_str(&t);
                    }
                    StreamMsg::Tool(label) => {
                        self.state.tools_used.lock().unwrap().push(label);
                    }
                    StreamMsg::Done => {
                        // Mover pending a messages
                        let text = std::mem::take(&mut *self.state.pending.lock().unwrap());
                        if !text.trim().is_empty() {
                            self.state.messages.lock().unwrap().push(ChatEntry::cephalon(text));
                        }
                        *self.state.thinking.lock().unwrap() = false;
                        // Audio: respuesta lista
                        if let Some(a) = &self.audio_player {
                            a.play(AudioEvent::NpcMessage {
                                npc: self.state.personality.lock().unwrap().npc(),
                                kind: NpcMessageKind::Arrived,
                            });
                        }
                    }
                    StreamMsg::Error(e) => {
                        // Consolidar pending si hay algo
                        let text = std::mem::take(&mut *self.state.pending.lock().unwrap());
                        if !text.trim().is_empty() {
                            self.state.messages.lock().unwrap().push(ChatEntry::cephalon(text));
                        }
                        self.state.messages.lock().unwrap().push(ChatEntry::error(e));
                        *self.state.thinking.lock().unwrap() = false;
                        if let Some(a) = &self.audio_player {
                            a.play(AudioEvent::Error);
                        }
                    }
                }
            }
        }
    }
}

impl eframe::App for CephalonGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll background cada frame
        self.poll_background();

        // Tema Warframe
        theme::apply(ctx);

        // Verificar quit flag
        if *self.state.quit.lock().unwrap() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Header
        self.render_header(ctx);

        // Central panel: split worldstate | chat
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                self.render_worldstate(&mut cols[0]);
                self.render_chat(&mut cols[1]);
            });
        });

        // Bottom HUD
        self.render_hud(ctx);

        // Re-paint continuo mientras streaming
        if *self.state.thinking.lock().unwrap() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

impl CephalonGui {
    fn render_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Logo
                if let Some(tex) = &self.logo_texture {
                    ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(48.0, 48.0)));
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("CEPHALON MATE").size(20.0).color(theme::GOLD).strong());
                    ui.label(egui::RichText::new("Asistente Warframe — IA Local (Ollama)").size(11.0).color(theme::DIM));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Personality selector
                    egui::ComboBox::from_label("")
                        .selected_text(format!("{:?}", *self.state.personality.lock().unwrap()))
                        .show_ui(ui, |ui| {
                            for p in [Personality::Ordis, Personality::Suda, Personality::Simaris] {
                                if ui.selectable_value(&mut *self.state.personality.lock().unwrap(), p, format!("{:?}", p)).changed() {
                                    // Cambiar personalidad
                                    let new_p = *self.state.personality.lock().unwrap();
                                    self.config.personality = new_p.to_string().to_lowercase();
                                    let _ = self.config.save(std::path::Path::new("config.toml"));
                                    // Recrear modelo si existe
                                    let model_name = format!("cehalon-{}", new_p.to_string().to_lowercase());
                                    if let Ok(models) = self.ollama.list_models() {
                                        if !models.iter().any(|m| m.starts_with(&model_name)) {
                                            // Crear modelo en background - usar referencia al client
                                            let ollama_host = self.ollama.host().to_string();
                                            let content = std::fs::read_to_string(format!("modelfiles/{}.Modelfile", new_p.to_string().to_lowercase())).ok();
                                            if let Some(content) = content {
                                                thread::spawn(move || {
                                                    let client = crate::brain::OllamaClient::new(ollama_host);
                                                    let _ = client.create_model(&model_name, &content);
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        });

                    // Ollama status
                    let ok = *self.state.ollama_ok.lock().unwrap();
                    ui.colored_label(if ok { theme::OK } else { theme::ERR }, if ok { "🟢 Ollama OK" } else { "🔴 Ollama OFF" });

                    // Model
                    ui.label(egui::RichText::new(format!("Model: {}", *self.state.model.lock().unwrap())).size(10.0).color(theme::DIM));
                });
            });
            ui.separator();
        });
    }

    fn render_worldstate(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(egui::RichText::new("🌍 WORLDSTATE").color(theme::GOLD).size(16.0));
            ui.separator();

            if let Some(ws) = &*self.state.worldstate.lock().unwrap() {
                // Fissures
                if !ws.fissures.is_empty() {
                    ui.label(egui::RichText::new("📍 FISURAS").color(theme::ACCENT).strong());
                    for f in &ws.fissures {
                        let mut tags = Vec::new();
                        if f.is_storm { tags.push("STORM"); }
                        if f.is_hard { tags.push("STEEL PATH"); }
                        let tag = if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(" ")) };
                        let remaining = WorldState::format_remaining(&f.expires_at);
                        ui.label(format!("  {} · {} · {}{} — {}", f.tier, f.node, f.mission_type, tag, remaining));
                    }
                    ui.add_space(8.0);
                }

                // Alerts
                if !ws.alerts.is_empty() {
                    ui.label(egui::RichText::new("⚠️ ALERTAS").color(theme::ACCENT).strong());
                    for a in &ws.alerts {
                        ui.label(format!("  {} · {} · {} — {}", a.node, a.mission_type, a.faction, a.reward_text));
                    }
                    ui.add_space(8.0);
                }

                // Sortie
                if let Some(s) = &ws.sortie {
                    ui.label(egui::RichText::new("🎯 SORTIE").color(theme::ACCENT).strong());
                    ui.label(format!("  Boss: {} · Faction: {}", s.boss, s.faction));
                    for m in &s.missions {
                        ui.label(format!("    {} · {} · {}", m.mission_type, m.modifier, m.node));
                    }
                    ui.add_space(8.0);
                }

                // Invasions
                if !ws.invasions.is_empty() {
                    ui.label(egui::RichText::new("⚔️ INVASIONES").color(theme::ACCENT).strong());
                    for i in &ws.invasions {
                        ui.label(format!(
                            "  {} · {} vs {} · {}% ({}/{})",
                            i.node,
                            i.attacker_faction,
                            i.defender_faction,
                            (i.completion * 100.0) as i32,
                            i.count,
                            i.required_runs
                        ));
                    }
                }
            } else {
                ui.label(egui::RichText::new("Cargando worldstate...").color(theme::DIM));
            }
        });
    }

    fn render_chat(&mut self, ui: &mut egui::Ui) {
        egui::TopBottomPanel::bottom("chat_input").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                // Local buffer for text edit (egui pattern)
                let mut input_buf = self.state.input.lock().unwrap().clone();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut input_buf)
                        .hint_text("Pregúntale a tu Cephalon... (/help)")
                        .desired_width(f32::INFINITY)
                        .frame(false)
                );
                // Sync back to shared state
                *self.state.input.lock().unwrap() = input_buf.clone();

                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.send_message();
                }
                // Tab autocomplete (simplificado)
                if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                    let input = input_buf.trim_start_matches('/').to_string();
                    let commands = ["/personality", "/model", "/status", "/worldstate", "/sync", "/clear", "/quit", "/help"];
                    let matches: Vec<&str> = commands.iter().filter(|c| c.trim_start_matches('/').starts_with(&input)).copied().collect();
                    if let Some(next) = matches.first() {
                        *self.state.input.lock().unwrap() = format!("{} ", next);
                    }
                }
            });
            ui.separator();
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let messages = self.state.messages.lock().unwrap();
                    for msg in messages.iter() {
                        let (color, prefix) = match msg.who {
                            crate::ui::ChatWho::You => (theme::USER_COLOR, "Tenno"),
                            crate::ui::ChatWho::Cephalon => (theme::GOLD, self.state.personality.lock().unwrap().name()),
                            crate::ui::ChatWho::Error => (theme::ERR, "ERROR"),
                        };
                        ui.horizontal(|ui| {
                            ui.colored_label(color, format!("{}: ", prefix));
                            ui.label(egui::RichText::new(&msg.text).color(theme::FG));
                        });
                        ui.add_space(4.0);
                    }

                    // Thinking indicator
                    if *self.state.thinking.lock().unwrap() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            let pending = self.state.pending.lock().unwrap();
                            if pending.is_empty() {
                                ui.label(egui::RichText::new("pensando...").color(theme::DIM).italics());
                            } else {
                                ui.label(egui::RichText::new(&*pending).color(theme::FG));
                            }
                        });
                    }

                    // Tools used line
                    let tools = self.state.tools_used.lock().unwrap();
                    if !tools.is_empty() {
                        ui.horizontal(|ui| {
                            ui.colored_label(theme::OK, format!(" ⚙ tools: {}", tools.join(" · ")));
                        });
                    }
                });
        });
    }

    fn render_hud(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("hud").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Status worldstate
                let ws_age = self.state.worldstate.lock().unwrap().as_ref().map(|ws| {
                    format!("{}s", ws.fetched_at.elapsed().as_secs())
                }).unwrap_or_else(|| String::from("sin datos"));
                ui.label(egui::RichText::new(format!("🌍 WS: {}", ws_age)).size(10.0).color(theme::DIM));

                ui.separator();

                // Mensajes count
                let count = self.state.messages.lock().unwrap().len();
                ui.label(egui::RichText::new(format!("💬 {}", count)).size(10.0).color(theme::DIM));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Enter: enviar  ·  Tab: completar  ·  Esc: salir").size(10.0).color(theme::DIM));
                    ui.separator();
                    ui.label(egui::RichText::new("Assets: Warframe Fansite Kit (Digital Extremes)  ·  Data: WFCD · warframe.market · warframestat.us").size(9.0).color(theme::DIM).italic());
                });
            });
        });
    }
}

/// Lanza la GUI (entry point desde main.rs).
pub fn run_gui(_config: Config) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Cephalon Mate"),
        ..Default::default()
    };
    eframe::run_native(
        "Cephalon Mate",
        options,
        Box::new(|cc| Ok(Box::new(CephalonGui::new(cc)) as Box<dyn eframe::App>)),
    )
}
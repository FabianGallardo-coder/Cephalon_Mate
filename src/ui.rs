//! Render del TUI: layout header / worldstate / chat / input / footer.
//!
//! Layout adaptable: el header cambia según el ancho (≥100 columnas muestra el
//! detalle completo, por debajo solo el nombre y la personalidad) y los
//! paneles pasan de lado a lado (≥90 columnas) a apilados en terminales
//! angostas.

use crate::audio::Personality;
use crate::brain::WorldState;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

/// Un mensaje visible en el panel de chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEntry {
    pub who: ChatWho,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatWho {
    You,
    Cephalon,
    /// Error mostrado en rojo (p. ej. Ollama caído).
    Error,
}

impl ChatEntry {
    pub fn you(text: impl Into<String>) -> Self {
        Self {
            who: ChatWho::You,
            text: text.into(),
        }
    }
    pub fn cephalon(text: impl Into<String>) -> Self {
        Self {
            who: ChatWho::Cephalon,
            text: text.into(),
        }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            who: ChatWho::Error,
            text: text.into(),
        }
    }
}

/// Estado que alimenta el render.
pub struct UiState {
    pub personality: String,
    pub model: String,
    pub ollama_host: String,
    pub ollama_ok: bool,
    /// Texto del input.
    pub input: String,
    /// Historial de mensajes del chat.
    pub messages: Vec<ChatEntry>,
    /// Spinner de "pensando…" mientras el modelo responde.
    pub thinking: bool,
    /// Texto parcial que llega por streaming (se pinta creciendo).
    pub pending: String,
    /// Etiquetas de tools ejecutadas en la ronda actual (línea verde).
    pub tools_used: Vec<String>,
    /// Progreso del sync WFCD (0-100), para mostrar en la barra de input sin pisar tools_used.
    pub wfcd_progress: Option<u8>,
    /// Worldstate cacheado para el panel izquierdo.
    pub worldstate: Option<WorldState>,
    /// Historial de envíos (flechas ↑/↓).
    pub history: Vec<String>,
    /// Índice en `history` recorrido con las flechas (None = en vivo).
    pub history_idx: Option<usize>,
    /// Estado del widget `List` del panel de worldstate.
    pub worldstate_list: ListState,
    /// Flag para salida limpia desde /quit.
    pub quit: bool,
}

impl UiState {
    pub fn new(personality: String, model: String, ollama_host: String, ollama_ok: bool) -> Self {
        Self {
            personality,
            model,
            ollama_host,
            ollama_ok,
            input: String::new(),
            messages: Vec::new(),
            thinking: false,
            pending: String::new(),
            tools_used: Vec::new(),
            wfcd_progress: None,
            worldstate: None,
            history: Vec::new(),
            history_idx: None,
            worldstate_list: ListState::default(),
            quit: false,
        }
    }

    /// Personalidad tipada (para elegir el set de audio).
    pub fn personality_enum(&self) -> Personality {
        Personality::from_str(&self.personality)
    }
}

// ── Paleta ────────────────────────────────────────────────────────────────
const BG: Color = Color::Rgb(8, 10, 14);
const FG: Color = Color::Rgb(220, 235, 240);
const ACCENT: Color = Color::Rgb(0, 229, 255); // cian Warframe
const DIM: Color = Color::Rgb(90, 110, 120);
const ALERT: Color = Color::Rgb(255, 152, 0); // naranja
const SORTIE: Color = Color::Rgb(255, 220, 60); // amarillo
const OK: Color = Color::Rgb(80, 220, 130); // verde tools
const PANEL: Color = Color::Rgb(18, 24, 32);

/// Ancho mínimo (en columnas) para el header detallado.
const HEADER_WIDE_WIDTH: u16 = 100;
/// Ancho mínimo (en columnas) para poner los paneles en columnas.
const PANELS_SIDE_BY_SIDE_WIDTH: u16 = 90;

fn accent() -> Style {
    Style::default().fg(ACCENT)
}

fn panel_block(title: &str) -> Block<'_> {
    Block::bordered()
        .border_style(Style::default().fg(Color::Rgb(0, 120, 140)))
        .style(Style::default().bg(PANEL).fg(FG))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
}

/// Punto de entrada del render. Se llama en cada `terminal.draw`.
///
/// Layout: 3% header + 97% cuerpo (paneles + input + footer).
pub fn render(state: &mut UiState, f: &mut Frame<'_>) {
    let area = f.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(3), Constraint::Percentage(97)])
        .split(area);

    render_header(state, f, rows[0]);

    // Cuerpo: paneles arriba, input y footer abajo (full width).
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(rows[1]);

    if area.width >= PANELS_SIDE_BY_SIDE_WIDTH {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(body[0]);
        render_worldstate(state, f, cols[0]);
        render_chat(state, f, cols[1]);
    } else {
        // Terminal angosto: worldstate arriba, chat abajo.
        let stacked = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(body[0]);
        render_worldstate(state, f, stacked[0]);
        render_chat(state, f, stacked[1]);
    }

    if body[1].height > 0 {
        render_input(state, f, body[1]);
    }
    if body[2].height > 0 {
        render_footer(f, body[2]);
    }
}

/// Header adaptable: detallado en terminales anchas, compacto en angostas.
pub fn render_header(state: &mut UiState, f: &mut Frame<'_>, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let status = if state.ollama_ok {
        Span::styled("● OK", Style::default().fg(OK))
    } else {
        Span::styled("● OFFLINE", Style::default().fg(Color::Rgb(255, 80, 80)))
    };

    let line = if area.width >= HEADER_WIDE_WIDTH {
        Line::from(vec![
            Span::styled(
                " ⚡ CEPHALON MATE ",
                Style::default().fg(BG).bg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("personality: ", Style::default().fg(DIM)),
            Span::styled(
                state.personality.to_uppercase(),
                accent().add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ·  model: ", Style::default().fg(DIM)),
            Span::raw(state.model.clone()),
            Span::styled("  ·  ollama: ", Style::default().fg(DIM)),
            Span::raw(state.ollama_host.clone()),
            Span::raw(" "),
            status,
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "⚡ CEPHALON MATE",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — ", Style::default().fg(DIM)),
            Span::styled(
                state.personality.to_uppercase(),
                accent().add_modifier(Modifier::BOLD),
            ),
        ])
        .centered()
    };

    f.render_widget(Paragraph::new(line).style(Style::default().bg(BG)), area);
}

/// Panel de worldstate (fissuras, alertas, sortie, invasiones).
pub fn render_worldstate(state: &mut UiState, f: &mut Frame<'_>, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let items: Vec<ListItem> = worldstate_items(state.worldstate.as_ref())
        .into_iter()
        .map(ListItem::new)
        .collect();

    let list = List::new(items)
        .block(panel_block("WORLDSTATE"))
        .style(Style::default().bg(PANEL).fg(FG));

    f.render_stateful_widget(list, area, &mut state.worldstate_list);
}

/// Construye las líneas del panel de worldstate.
fn worldstate_items(ws: Option<&WorldState>) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();

    let Some(ws) = ws else {
        out.push(Line::styled(
            " WORLDSTATE · esperando primer fetch…",
            Style::default().fg(DIM),
        ));
        return out;
    };

    out.push(Line::styled(
        format!(" actualizado hace {}s", ws.fetched_at.elapsed().as_secs()),
        Style::default().fg(OK),
    ));

    out.push(Line::styled(
        " ▓ FISSURES",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ));
    if ws.fissures.is_empty() {
        out.push(Line::styled("  (ninguna activa)", Style::default().fg(DIM)));
    } else {
        for f in &ws.fissures {
            let remaining = WorldState::format_remaining(&f.expires_at);
            let tag = if f.is_storm {
                " [STORM]"
            } else if f.is_hard {
                " [STEEL PATH]"
            } else {
                ""
            };
            let line = format!("  - {} · {}{}", f.tier, f.node, f.mission_type);
            out.push(Line::styled(format!("{line}{tag}"), Style::default().fg(FG)));
            if !remaining.is_empty() {
                out.push(Line::styled(
                    format!("    · {remaining}"),
                    Style::default().fg(SORTIE),
                ));
            }
        }
    }

    out.push(Line::styled(
        " ⚠ ALERTS",
        Style::default().fg(ALERT).add_modifier(Modifier::BOLD),
    ));
    if ws.alerts.is_empty() {
        out.push(Line::styled("  (ninguna activa)", Style::default().fg(DIM)));
    } else {
        for a in &ws.alerts {
            let remaining = WorldState::format_remaining(&a.expires_at);
            let line = format!(
                "  - {} · {} · {} ({})",
                a.mission_type, a.node, a.faction, a.reward_text
            );
            out.push(Line::styled(line, Style::default().fg(FG)));
            if !remaining.is_empty() {
                out.push(Line::styled(
                    format!("    · {remaining}"),
                    Style::default().fg(SORTIE),
                ));
            }
        }
    }

    out.push(Line::styled(
        " ★ SORTIE",
        Style::default().fg(SORTIE).add_modifier(Modifier::BOLD),
    ));
    match &ws.sortie {
        Some(s) => {
            for (i, m) in s.missions.iter().enumerate() {
                out.push(Line::styled(
                    format!("  {}. {} · {} ({})", i + 1, m.mission_type, m.node, m.modifier),
                    Style::default().fg(FG),
                ));
            }
            out.push(Line::styled(
                format!("  Boss: {} ({})", s.boss, s.faction),
                Style::default().fg(FG),
            ));
            let remaining = WorldState::format_remaining(&s.expires_at);
            if !remaining.is_empty() {
                out.push(Line::styled(format!("    {remaining}"), Style::default().fg(SORTIE)));
            }
        }
        None => out.push(Line::styled("  (sin sortie activa)", Style::default().fg(DIM))),
    }

    out.push(Line::styled(
        " ☮ INVASIONS",
        Style::default().fg(OK).add_modifier(Modifier::BOLD),
    ));
    if ws.invasions.is_empty() {
        out.push(Line::styled("  (ninguna activa)", Style::default().fg(DIM)));
    } else {
        for i in &ws.invasions {
            let pct = (i.completion * 100.0).round() as i64;
            let status_str = if i.completed {
                "✓ COMPLETADA"
            } else {
                &format!("{pct}%")
            };
            let line = format!(
                "  - {} · {} ({}) vs {} ({}) · {}",
                i.desc,
                i.attacker_faction,
                i.attacker_reward,
                i.defender_faction,
                i.defender_reward,
                status_str
            );
            out.push(Line::styled(line, Style::default().fg(FG)));
        }
    }

    out
}

/// Panel de chat.
pub fn render_chat(state: &mut UiState, f: &mut Frame<'_>, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let block = panel_block("CHAT");
    let inner_w = area.width.saturating_sub(2) as usize;

    // Construir todas las líneas del chat (etiqueta + texto envuelto).
    let mut lines: Vec<Line> = Vec::new();
    if state.messages.is_empty() {
        lines.push(Line::styled(
            " (vacío — escribe «hola» para despertar a tu Cephalon)",
            Style::default().fg(DIM),
        ));
    }
    for msg in &state.messages {
        match msg.who {
            ChatWho::You => {
                lines.push(Line::styled(
                    " TÚ",
                    Style::default().fg(DIM).add_modifier(Modifier::BOLD),
                ));
                for l in wrap(&msg.text, inner_w) {
                    lines.push(Line::styled(format!(" {l}"), Style::default().fg(FG)));
                }
            }
            ChatWho::Cephalon => {
                let tag = format!(" ◆ {}", state.personality.to_uppercase());
                lines.push(Line::styled(
                    tag,
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ));
                for l in wrap(&msg.text, inner_w) {
                    lines.push(Line::styled(format!(" {l}"), Style::default().fg(FG)));
                }
            }
            ChatWho::Error => {
                lines.push(Line::styled(
                    " ✗ ERROR",
                    Style::default().fg(Color::Rgb(255, 80, 80)).add_modifier(Modifier::BOLD),
                ));
                for l in wrap(&msg.text, inner_w) {
                    lines.push(Line::styled(
                        format!(" {l}"),
                        Style::default().fg(Color::Rgb(255, 120, 120)),
                    ));
                }
            }
        }
        lines.push(Line::raw(""));
    }

    // Respuesta en curso: texto parcial creciendo + cursor.
    let is_streaming = !state.pending.is_empty() || state.thinking || !state.tools_used.is_empty() || state.wfcd_progress.is_some();
    if is_streaming {
        lines.push(Line::styled(
            format!(" ◆ {}", state.personality.to_uppercase()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
        if state.pending.is_empty() && !state.tools_used.is_empty() {
            // Aún no llegó texto: mostrar el spinner mientras corre la tool.
            lines.push(Line::styled(" ⠋ consultando…", Style::default().fg(DIM)));
        } else if state.pending.is_empty() && state.wfcd_progress.is_some() {
            // Progreso de WFCD sync.
            if let Some(pct) = state.wfcd_progress {
                lines.push(Line::styled(format!(" ⠋ sincronizando WFCD… {}%", pct), Style::default().fg(DIM)));
            }
        } else if state.pending.is_empty() {
            lines.push(Line::styled(" ⠋ pensando…", Style::default().fg(DIM)));
        } else {
            for l in wrap(&state.pending, inner_w) {
                lines.push(Line::styled(format!(" {l}"), Style::default().fg(FG)));
            }
            lines.push(Line::styled(" ▌", accent()));
        }
        // Línea verde de tools ejecutadas (transparencia anti-alucinación).
        if !state.tools_used.is_empty() {
            lines.push(Line::styled(
                format!(" ⚙ tools: {}", state.tools_used.join(" · ")),
                Style::default().fg(OK),
            ));
        }
    }

    // Auto-scroll: mostrar solo las últimas `max_lines` que caben.
    let max_lines = area.height.saturating_sub(2) as usize;
    let start = lines.len().saturating_sub(max_lines);
    let visible: Vec<Line> = lines.into_iter().skip(start).collect();

    f.render_widget(
        Paragraph::new(visible)
            .block(block)
            .style(Style::default().bg(BG).fg(FG)),
        area,
    );
}

/// Envoltura de palabras simple a `width` columnas (sin romper palabras).
fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.is_empty() {
                line.push_str(word);
            } else if line.chars().count() + 1 + word.chars().count() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        out.push(line);
    }
    out
}

fn render_input(state: &mut UiState, f: &mut Frame<'_>, area: Rect) {
    let placeholder = "Pregúntale a tu Cephalon... ( /personality  /status  /worldstate )";
    let content = if state.input.is_empty() {
        Line::from(vec![
            Span::styled("> ", accent().add_modifier(Modifier::BOLD)),
            Span::styled(placeholder, Style::default().fg(DIM)),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", accent().add_modifier(Modifier::BOLD)),
            Span::raw(state.input.clone()),
            Span::styled("▌", accent()),
        ])
    };

    let block = Block::bordered()
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
    f.render_widget(Paragraph::new(content).block(block), area);
}

fn render_footer(f: &mut Frame<'_>, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" Enter: enviar", Style::default().fg(DIM)),
        Span::styled("  ·  ", Style::default().fg(Color::Rgb(50, 60, 70))),
        Span::styled("/help", Style::default().fg(DIM)),
        Span::styled("  ·  ", Style::default().fg(Color::Rgb(50, 60, 70))),
        Span::styled("Tab: completar", Style::default().fg(DIM)),
        Span::styled("  ·  ", Style::default().fg(Color::Rgb(50, 60, 70))),
        Span::styled("Ctrl+C: salir", Style::default().fg(DIM)),
        Span::styled("  ·  ", Style::default().fg(Color::Rgb(50, 60, 70))),
        Span::styled("Assets: Warframe Fansite Kit (Digital Extremes)", Style::default().fg(Color::Rgb(100, 120, 140)).italic()),
        Span::styled("  ·  ", Style::default().fg(Color::Rgb(50, 60, 70))),
        Span::styled("Data: WFCD · warframe.market · warframestat.us", Style::default().fg(Color::Rgb(100, 120, 140)).italic()),
    ]);
    f.render_widget(Paragraph::new(line).style(Style::default().bg(BG)), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_at(w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut state = UiState::new(
            "ordis".into(),
            "qwen2.5:3b".into(),
            "http://127.0.0.1:12434".into(),
            true,
        );
        term.draw(|f| render(&mut state, f)).unwrap();
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

    /// Done de P04: el layout completo se renderiza sin panic.
    #[test]
    fn renders_full_layout() {
        let out = render_at(120, 30);
        assert!(out.contains("CEPHALON MATE"), "falta el header");
        assert!(out.contains("WORLDSTATE"), "falta el panel izquierdo");
        assert!(out.contains("CHAT"), "falta el panel de chat");
        assert!(out.contains("Pregúntale a tu Cephalon"), "falta el input");
        assert!(out.contains("Enter: enviar"), "falta el footer");
    }

    /// Done de P04: reacciona a resize sin romperse (incluido muy pequeño).
    #[test]
    fn survives_resize() {
        for (w, h) in [(20u16, 6u16), (60, 15), (160, 50)] {
            let _ = render_at(w, h); // no debe panic
        }
    }

    /// Header detallado solo en terminales anchas (≥100 columnas).
    #[test]
    fn header_is_adaptive() {
        let wide = render_at(120, 30);
        assert!(wide.contains("personality:"), "header ancho sin detalle");
        assert!(wide.contains("model:"), "header ancho sin modelo");
        assert!(wide.contains("ollama:"), "header ancho sin host");
        assert!(wide.contains("● OK"), "header ancho sin estado");

        let narrow = render_at(80, 30);
        assert!(narrow.contains("CEPHALON MATE"), "header angosto sin nombre");
        assert!(!narrow.contains("personality:"), "header angosto con detalle");
        assert!(narrow.contains("ORDIS"), "header angosto sin personalidad");
    }

    /// En terminales angostas los paneles se apilan (worldstate + chat).
    #[test]
    fn narrow_layout_keeps_both_panels() {
        let out = render_at(70, 40);
        assert!(out.contains("WORLDSTATE"), "falta worldstate en angosto");
        assert!(out.contains("CHAT"), "falta chat en angosto");
    }
}

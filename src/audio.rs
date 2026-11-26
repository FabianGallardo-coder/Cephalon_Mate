//! Audio playback system using rodio — reproducción de sonidos del Fansite Kit.
//!
//! P13 del `PLAN_EJECUCION.md`: thread de audio no bloqueante con canal `mpsc`.
//! Los sonidos viven en subcarpetas por NPC (`Audio/NPCs/Lotus/`,
//! `Audio/NPCs/Ordis/`), así que las rutas se resuelven en runtime contra
//! `audio_dir()` en vez de asumir una carpeta plana.

use std::path::{Path, PathBuf};

use rodio::{Decoder, OutputStream, Sink};

/// Eventos de audio que el event loop puede enviar.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Nuevo mensaje del Cephalon (personalidad-dependiente).
    CephalonMessage { personality: Personality },
    /// Mensaje de un NPC concreto con un tipo de evento.
    NpcMessage { npc: Npc, kind: NpcMessageKind },
    /// Worldstate actualizado (sonido según la personalidad activa).
    WorldstateUpdated { personality: Personality },
    /// Tool ejecutada correctamente.
    ToolExecuted { success: bool },
    /// Error genérico.
    Error,
    /// Alerta crítica (fissura Steel Path, sortie nueva, etc.).
    CriticalAlert { personality: Personality },
    /// Spinner "pensando..." (loop corto para Ordis).
    #[allow(dead_code)]
    SpinnerTick,
    /// Apagar el thread de audio.
    Shutdown,
}

/// Personalidad activa (determina qué sonidos usar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Personality {
    Ordis,
    Suda,
    Simaris,
}

impl Personality {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ordis" => Personality::Ordis,
            "suda" => Personality::Suda,
            "simaris" => Personality::Simaris,
            _ => Personality::Ordis,
        }
    }

    /// NPC cuyo set de sonidos corresponde a esta personalidad.
    ///
    /// Ordis usa su propio set; Suda y Simaris comparten la voz de Lotus.
    pub fn npc(&self) -> Npc {
        match self {
            Personality::Ordis => Npc::Ordis,
            Personality::Suda | Personality::Simaris => Npc::Lotus,
        }
    }
}

/// NPC del Fansite Kit con su carpeta de sonidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Npc {
    Lotus,
    Ordis,
}

impl Npc {
    /// Carpeta del NPC dentro de `Audio/NPCs/`.
    #[allow(dead_code)]
    pub fn dir(&self) -> &'static str {
        match self {
            Npc::Lotus => "Lotus",
            Npc::Ordis => "Ordis",
        }
    }
}

/// Tipo de mensaje que dispara un sonido de NPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcMessageKind {
    /// El NPC inicia una comunicación.
    Incoming,
    /// El NPC está esperando respuesta.
    Waiting,
    /// Llegó un mensaje nuevo.
    Arrived,
    /// Mensaje recibido pero sin atención inmediata.
    WaitingMessage,
    /// Alarma / Something's happening.
    Alert,
}

/// Archivos de audio usados por la app, con su ruta relativa exacta dentro
/// de la carpeta `Audio/` del Fansite Kit.
pub static AUDIO_FILES: &[&str] = &[
    // ── Ordis ──────────────────────────────────────────────────────────────
    "NPCs/Ordis/Ordis - Incoming Call 1.mp3",
    "NPCs/Ordis/Ordis - Incoming Call 2.mp3",
    "NPCs/Ordis/Ordis - Incoming Call 3.mp3",
    "NPCs/Ordis/Ordis - Incoming Call or Message.mp3",
    "NPCs/Ordis/Ordis - Incoming Message 1.mp3",
    "NPCs/Ordis/Ordis - Incoming Message 2.mp3",
    "NPCs/Ordis/Ordis - Alarm (Emergency).mp3",
    "NPCs/Ordis/Ordis - Alarm (System in Need).mp3",
    "NPCs/Ordis/Ordis - Alarm (Wake Up).mp3",
    "NPCs/Ordis/OrdisIsItTalking.mp3",
    // ── Lotus (Suda / Simaris) ─────────────────────────────────────────────
    "NPCs/Lotus/Lotus_Incoming Communication.mp3",
    "NPCs/Lotus/Lotus_Incoming Transmission.mp3",
    "NPCs/Lotus/Lotus_Message Arrived.mp3",
    "NPCs/Lotus/Lotus_Message Recieved.mp3",
    "NPCs/Lotus/Lotus_Message Waiting.mp3",
    "NPCs/Lotus/Lotus_Somethings Happening Tenno.mp3",
    "NPCs/Lotus/Lotus_It Is Time Tenno.mp3",
    "NPCs/Lotus/Lotus_Transmission Recieved.mp3",
    "NPCs/Lotus/Lotus_Wake Up Tenno.mp3",
    // ── UI genérica ────────────────────────────────────────────────────────
    "Miscellaneous/Alarm.mp3",
    "Miscellaneous/CipherFail.mp3",
    "Miscellaneous/EnergyPickup.mp3",
    "Miscellaneous/ShipPagerDing.mp3",
    "Miscellaneous/SingleDrumHit.mp3",
];

/// Archivo que suena para un (NPC, tipo de mensaje) concreto.
pub fn npc_file(npc: Npc, kind: NpcMessageKind) -> &'static str {
    match (npc, kind) {
        (Npc::Ordis, NpcMessageKind::Incoming) => "NPCs/Ordis/Ordis - Incoming Call 1.mp3",
        (Npc::Ordis, NpcMessageKind::Waiting) => "NPCs/Ordis/Ordis - Incoming Call 2.mp3",
        (Npc::Ordis, NpcMessageKind::Arrived) => "NPCs/Ordis/Ordis - Incoming Message 1.mp3",
        (Npc::Ordis, NpcMessageKind::WaitingMessage) => "NPCs/Ordis/Ordis - Incoming Message 2.mp3",
        (Npc::Ordis, NpcMessageKind::Alert) => "NPCs/Ordis/Ordis - Alarm (Emergency).mp3",
        (Npc::Lotus, NpcMessageKind::Incoming) => "NPCs/Lotus/Lotus_Incoming Communication.mp3",
        (Npc::Lotus, NpcMessageKind::Waiting) => "NPCs/Lotus/Lotus_Message Waiting.mp3",
        (Npc::Lotus, NpcMessageKind::Arrived) => "NPCs/Lotus/Lotus_Message Arrived.mp3",
        (Npc::Lotus, NpcMessageKind::WaitingMessage) => "NPCs/Lotus/Lotus_Message Recieved.mp3",
        (Npc::Lotus, NpcMessageKind::Alert) => "NPCs/Lotus/Lotus_Somethings Happening Tenno.mp3",
    }
}

/// Sonido de "worldstate actualizado" según la personalidad activa.
pub fn worldstate_file(npc: Npc) -> &'static str {
    match npc {
        Npc::Ordis => "NPCs/Ordis/Ordis - Incoming Call or Message.mp3",
        Npc::Lotus => "NPCs/Lotus/Lotus_Transmission Recieved.mp3",
    }
}

/// Sonido de "alerta crítica" según la personalidad activa.
pub fn critical_file(npc: Npc) -> &'static str {
    match npc {
        Npc::Ordis => "NPCs/Ordis/Ordis - Alarm (System in Need).mp3",
        Npc::Lotus => "NPCs/Lotus/Lotus_It Is Time Tenno.mp3",
    }
}

/// Carpeta `Audio/` del Fansite Kit.
///
/// Se busca, en orden: junto al ejecutable (`<exe>/Audio`), junto al
/// ejecutable dentro del Kit (`<exe>/Warframe Fansite Kit/Audio`), y subiendo
/// desde el directorio actual hasta encontrar el Kit del proyecto.
pub fn audio_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(found) = probe_audio_dir(dir) {
                return found;
            }
        }
    }

    let mut current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if let Some(found) = probe_audio_dir(&current) {
            return found;
        }
        if !current.pop() {
            break;
        }
    }

    // Fallback relativo (por si el kit se copia junto al ejecutable después).
    PathBuf::from("Warframe Fansite Kit").join("Audio")
}

/// Busca la carpeta de audio dentro de `base`.
fn probe_audio_dir(base: &Path) -> Option<PathBuf> {
    let direct = base.join("Audio");
    if direct.is_dir() {
        return Some(direct);
    }
    let in_kit = base.join("Warframe Fansite Kit").join("Audio");
    if in_kit.is_dir() {
        return Some(in_kit);
    }
    None
}

/// Reproductor de audio en thread aparte.
#[derive(Clone)]
pub struct AudioPlayer {
    sender: std::sync::Arc<std::sync::mpsc::Sender<AudioEvent>>,
    handle: std::sync::Arc<std::sync::Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl AudioPlayer {
    /// Inicia el thread de audio y precarga los sonidos.
    /// `kit_root` es la ruta a `Audio/` del Fansite Kit.
    pub fn new(kit_root: PathBuf) -> Result<Self, String> {
        let (tx, rx) = std::sync::mpsc::channel();

        let handle = std::thread::spawn(move || {
            Self::run_audio_thread(rx, kit_root);
        });

        Ok(Self {
            sender: std::sync::Arc::new(tx),
            handle: std::sync::Arc::new(std::sync::Mutex::new(Some(handle))),
        })
    }

    /// Envía un evento de audio (no bloquea).
    pub fn play(&self, event: AudioEvent) {
        let _ = self.sender.send(event);
    }

    /// Detiene el thread de audio al salir.
    pub fn shutdown(&mut self) {
        let _ = self.sender.send(AudioEvent::Shutdown);
        if let Ok(mut handle) = self.handle.lock() {
            if let Some(h) = handle.take() {
                let _ = h.join();
            }
        }
    }

    /// Thread principal de audio: precarga sonidos y procesa eventos.
    fn run_audio_thread(rx: std::sync::mpsc::Receiver<AudioEvent>, kit_root: PathBuf) {
        // Configurar salida de audio - rodio 0.19 usa OutputStream::try_default()
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[audio] No se pudo abrir dispositivo de audio: {e}");
                return;
            }
        };
        let sink = match Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[audio] No se pudo crear sink: {e}");
                return;
            }
        };

        // Precargar sonidos del Kit
        let sounds = Self::load_sounds(&kit_root);
        if sounds.is_empty() {
            eprintln!("[audio] No se cargaron sonidos del Kit (ruta: {:?})", kit_root);
        } else {
            println!("[audio] Cargados {} sonidos del Kit", sounds.len());
        }

        // Volumen base (0.0 - 1.0)
        sink.set_volume(0.5);

        // Loop de eventos
        loop {
            match rx.recv() {
                Ok(event) => match event {
                    AudioEvent::CephalonMessage { personality } => {
                        let npc = personality.npc();
                        Self::play_sound(&sink, &sounds, npc_file(npc, NpcMessageKind::Incoming));
                    }
                    AudioEvent::NpcMessage { npc, kind } => {
                        Self::play_sound(&sink, &sounds, npc_file(npc, kind));
                    }
                    AudioEvent::WorldstateUpdated { personality } => {
                        Self::play_sound(&sink, &sounds, worldstate_file(personality.npc()));
                    }
                    AudioEvent::ToolExecuted { success } => {
                        let file = if success {
                            "Miscellaneous/EnergyPickup.mp3"
                        } else {
                            "Miscellaneous/CipherFail.mp3"
                        };
                        Self::play_sound(&sink, &sounds, file);
                    }
                    AudioEvent::Error => {
                        Self::play_sound(&sink, &sounds, "Miscellaneous/CipherFail.mp3");
                    }
                    AudioEvent::CriticalAlert { personality } => {
                        Self::play_sound(&sink, &sounds, critical_file(personality.npc()));
                    }
                    AudioEvent::SpinnerTick => {
                        Self::play_sound(&sink, &sounds, "Miscellaneous/SingleDrumHit.mp3");
                    }
                    AudioEvent::Shutdown => {
                        sink.sleep_until_end();
                        break;
                    }
                },
                Err(_) => break, // canal cerrado
            }
        }
    }

    /// Carga sonidos del Kit en memoria (HashMap<ruta relativa, bytes>).
    fn load_sounds(kit_root: &Path) -> std::collections::HashMap<String, Vec<u8>> {
        let mut map = std::collections::HashMap::new();

        for rel_path in AUDIO_FILES {
            let full_path = kit_root.join(rel_path);
            if full_path.exists() {
                match std::fs::read(&full_path) {
                    Ok(bytes) => {
                        map.insert((*rel_path).to_string(), bytes);
                    }
                    Err(e) => {
                        eprintln!("[audio] Error leyendo {}: {}", full_path.display(), e);
                    }
                }
            } else {
                eprintln!("[audio] Archivo no encontrado: {}", full_path.display());
            }
        }

        map
    }

    /// Reproduce un sonido por ruta relativa dentro de `Audio/`.
    fn play_sound(sink: &Sink, sounds: &std::collections::HashMap<String, Vec<u8>>, rel_path: &str) {
        if let Some(bytes) = sounds.get(rel_path) {
            if let Ok(source) = Decoder::new(std::io::Cursor::new(bytes.clone())) {
                sink.append(source);
            }
        }
    }
}

/// Inicializa el reproductor usando la carpeta `Audio/` resuelta en runtime.
pub fn init_audio() -> std::io::Result<AudioPlayer> {
    AudioPlayer::new(audio_dir()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personality_from_str() {
        assert_eq!(Personality::from_str("ordis"), Personality::Ordis);
        assert_eq!(Personality::from_str("suda"), Personality::Suda);
        assert_eq!(Personality::from_str("simaris"), Personality::Simaris);
        assert_eq!(Personality::from_str("ORdis"), Personality::Ordis);
        assert_eq!(Personality::from_str("unknown"), Personality::Ordis);
    }

    #[test]
    fn personality_maps_to_npc() {
        assert_eq!(Personality::Ordis.npc(), Npc::Ordis);
        assert_eq!(Personality::Suda.npc(), Npc::Lotus);
        assert_eq!(Personality::Simaris.npc(), Npc::Lotus);
    }

    #[test]
    fn npc_dirs_are_subfolders() {
        assert_eq!(Npc::Lotus.dir(), "Lotus");
        assert_eq!(Npc::Ordis.dir(), "Ordis");
    }

    #[test]
    fn every_npc_mapping_points_to_a_known_file() {
        for npc in [Npc::Lotus, Npc::Ordis] {
            for kind in [
                NpcMessageKind::Incoming,
                NpcMessageKind::Waiting,
                NpcMessageKind::Arrived,
                NpcMessageKind::WaitingMessage,
                NpcMessageKind::Alert,
            ] {
                let file = npc_file(npc, kind);
                assert!(
                    AUDIO_FILES.contains(&file),
                    "{file:?} no está en AUDIO_FILES"
                );
                assert!(
                    file.contains(npc.dir()),
                    "{file:?} debería vivir en la carpeta del NPC {:?}",
                    npc.dir()
                );
            }
        }
    }

    #[test]
    fn worldstate_and_critical_sounds_are_known_files() {
        for npc in [Npc::Lotus, Npc::Ordis] {
            assert!(AUDIO_FILES.contains(&worldstate_file(npc)));
            assert!(AUDIO_FILES.contains(&critical_file(npc)));
        }
    }

    #[test]
    fn audio_files_are_relative_and_grouped() {
        for file in AUDIO_FILES {
            assert!(!Path::new(file).is_absolute(), "{file} debería ser relativa");
            assert!(
                file.starts_with("NPCs/") || file.starts_with("Miscellaneous/"),
                "{file} debería vivir en NPCs/ o Miscellaneous/"
            );
        }
    }

    #[test]
    fn audio_dir_points_to_audio_folder() {
        let dir = audio_dir();
        assert!(
            dir.ends_with("Audio"),
            "audio_dir() debería terminar en Audio: {dir:?}"
        );
    }
}

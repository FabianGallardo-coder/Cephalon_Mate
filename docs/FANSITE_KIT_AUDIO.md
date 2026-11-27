# Catálogo de Audio — Warframe Fansite Kit

> Ubicación: `C:\Users\Fabian\Desktop\_Proyectos\Cephalon_Mate\Warframe Fansite Kit\Audio\`
> Total: **482 archivos** organizados en carpetas temáticas.

---

## Estructura por carpetas

| Carpeta | Subcarpetas | Archivos | Uso sugerido |
|---------|-------------|----------|--------------|
| **NPCs/** | 8 (Ordis, Lotus, Darvo, Enemies, Hunhow, Maroo, Misc, Nora Night) | ~200 | **Core TUI** — voces de Cephalons |
| **Warframe Abilities/** | 1 (plano) | ~150 | FX de skills (opcional) |
| **Miscellaneous/** | 2 (Lotus, Files for iPhone) | ~10 | UI sounds genéricos |
| **JoeyZero Songs/** | — | — | Música (no usar en TUI) |
| **Requests/** | — | — | — |
| **AnimalCalls/** | — | — | — |

---

## Mapeo por Personalidad (TUI)

### 🟢 **Ordis** (`Audio/NPCs/Ordis/` — 20 archivos)
| Evento TUI | Archivo candidato | Duración approx | Notas |
|------------|-------------------|-----------------|-------|
| Mensaje nuevo del Cephalon | `Ordis - Incoming Call 1.mp3` (50 KB) | ~1.5s | Corto, claro |
| Tool ejecutada | `Ordis - Incoming Message 1.mp3` (98 KB) | ~3s | "Tenno, incoming data" |
| Error / fallo | `Ordis - Alarm (Emergency).mp3` (199 KB) | ~6s | Dramático |
| Worldstate actualizado | `Ordis - Incoming Call or Message.mp3` (103 KB) | ~3s | Neutro |
| Spinner "pensando" | `OrdisIsItTalking.mp3` (153 KB) | ~4.5s | Loopable |
| Alerta crítica (fissura steel path) | `Ordis - Alarm (System in Need).mp3` (57 KB) | ~2s | Urgente |
| Engage/Purge (comando especial) | `OrdisEngagePurgePrecept.mp3` (138 KB) | ~4s | Épico |

### 🔵 **Suda / Simaris** (`Audio/NPCs/Lotus/` — 10 archivos)
*Usar voz de Lotus como base (no hay carpeta Suda/Simaris en el kit)*

| Evento TUI | Archivo candidato | Duración approx | Notas |
|------------|-------------------|-----------------|-------|
| Mensaje nuevo | `Lotus_Incoming Audio Transmission.mp3` (83 KB) | ~2.5s | Limpio |
| Tool ejecutada | `Lotus_Message Arrived.mp3` (62 KB) | ~2s | Preciso |
| Worldstate actualizado | `Lotus_Transmission Recieved.mp3` (67 KB) | ~2s | Neutro |
| Alerta (sortie/invasion) | `Lotus_It Is Time Tenno.mp3` (72 KB) | ~2s | Solemne |
| Alerta crítica | `Lotus_Somethings Happening Tenno.mp3` (76 KB) | ~2.5s | Urgente |

### ⚪ **UI Genérico** (`Audio/Miscellaneous/` + `Misc/`)
| Evento TUI | Archivo candidato | Uso |
|------------|-------------------|-----|
| Keystroke / tick | `SingleDrumHit.mp3` | Input feedback sutil |
| Notificación éxito | `EnergyPickup.mp3` | Tool completada OK |
| Error / fallo | `CipherFail.mp3` | Tool falló / error red |
| Alarma / atención | `Alarm.mp3` | Worldstate refresh / alerta |
| Ping / heartbeat | `ShipPagerDing.mp3` | Worldstate actualizado |

---

## Implementación técnica (P13)

> **Estado:** lo siguiente es el diseño propuesto en P13. La implementación
> real está en **`src/audio.rs`** (thread con `mpsc`, precarga en
> `AUDIO_FILES`, rutas resueltas en runtime con `audio_dir()` — no se copia
> nada a `data/audio/` ni se usan rutas absolutas).

### Requisitos
- `rodio` crate para playback de audio (soporta .mp3, .wav, .ogg)
- `std::sync::mpsc` channel desde event loop → audio thread
- **No bloquear el event loop** — audio en thread aparte

### Estructura propuesta

```rust
// brain/audio.rs
pub enum AudioEvent {
    CephalonMessage { personality: Personality },
    ToolExecuted { success: bool },
    WorldstateUpdated,
    Error,
    CriticalAlert,
    SpinnerTick,
}

pub fn spawn_audio_player(rx: Receiver<AudioEvent>) {
    let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&handle).unwrap();
    // precargar sonidos en HashMap<&'static str, Vec<u8>>
    loop {
        match rx.recv() {
            Ok(evt) => play_for_event(&sink, evt),
            Err(_) => break,
        }
    }
}
```

### Mapeo `AudioEvent` → archivo por personalidad

```rust
fn sound_for(event: AudioEvent, personality: Personality) -> &'static str {
    match (event, personality) {
        (AudioEvent::CephalonMessage, Personality::Ordis) => "Ordis - Incoming Call 1.mp3",
        (AudioEvent::ToolExecuted { success: true }, Personality::Ordis) => "Ordis - Incoming Message 1.mp3",
        (AudioEvent::ToolExecuted { success: false }, Personality::Ordis) => "Ordis - Alarm (Emergency).mp3",
        (AudioEvent::WorldstateUpdated, Personality::Ordis) => "Ordis - Incoming Call or Message.mp3",
        (AudioEvent::CriticalAlert, Personality::Ordis) => "Ordis - Alarm (System in Need).mp3",
        
        (AudioEvent::CephalonMessage, Personality::Suda) => "Lotus_Incoming Audio Transmission.mp3",
        (AudioEvent::ToolExecuted { success: true }, Personality::Suda) => "Lotus_Message Arrived.mp3",
        // ... etc para Simaris (usar misma base Lotus pero distinto pitch/volumen)
        
        (AudioEvent::Error, _) => "CipherFail.mp3",
        (AudioEvent::SpinnerTick, Personality::Ordis) => "OrdisIsItTalking.mp3",
    }
}
```

### Precarga al arrancar (en `main.rs`)
```rust
// Copiar archivos del Kit a `data/audio/` (solo los 30-40 que usamos)
fn copy_audio_assets() -> Result<HashMap<&'static str, Vec<u8>>, String> {
    let kit_root = Path::new(r"C:\Users\Fabian\Desktop\_Proyectos\Cephalon_Mate\Warframe Fansite Kit\Audio");
    let map = [
        ("ordis_incoming_call_1", kit_root.join("NPCs/Ordis/Ordis - Incoming Call 1.mp3")),
        ("ordis_incoming_msg_1", kit_root.join("NPCs/Ordis/Ordis - Incoming Message 1.mp3")),
        ("ordis_alarm_emergency", kit_root.join("NPCs/Ordis/Ordis - Alarm (Emergency).mp3")),
        ("ordis_alarm_system", kit_root.join("NPCs/Ordis/Ordis - Alarm (System in Need).mp3")),
        ("ordis_engage", kit_root.join("NPCs/Ordis/OrdisEngagePurgePrecept.mp3")),
        ("ordis_talking", kit_root.join("NPCs/Ordis/OrdisIsItTalking.mp3")),
        ("lotus_transmission", kit_root.join("NPCs/Lotus/Lotus_Incoming Audio Transmission.mp3")),
        ("lotus_msg_arrived", kit_root.join("NPCs/Lotus/Lotus_Message Arrived.mp3")),
        ("lotus_time_tenno", kit_root.join("NPCs/Lotus/Lotus_It Is Time Tenno.mp3")),
        ("lotus_something", kit_root.join("NPCs/Lotus/Lotus_Somethings Happening Tenno.mp3")),
        ("ui_alarm", kit_root.join("Miscellaneous/Alarm.mp3")),
        ("ui_cipher_fail", kit_root.join("Miscellaneous/CipherFail.mp3")),
        ("ui_energy_pickup", kit_root.join("Miscellaneous/EnergyPickup.mp3")),
        ("ui_ship_pager", kit_root.join("Miscellaneous/ShipPagerDing.mp3")),
        ("ui_single_drum", kit_root.join("Miscellaneous/SingleDrumHit.mp3")),
    ].into_iter()
        .filter_map(|(k, p)| std::fs::read(p).ok().map(|b| (k, b)))
        .collect();
    Ok(map)
}
```

---

## Verificación manual (checklist)

- [x] `rodio` añadido a `Cargo.toml`
- [x] Thread de audio arranca en `main.rs` antes del event loop
- [x] Canal `mpsc::Sender<AudioEvent>` pasado al event loop
- [x] Eventos emitidos en puntos clave:
  - [x] `send_chat` → `CephalonMessage` (+ `NpcMessage Waiting` en cola)
  - [x] Tool `Done` → `ToolExecuted { success: true }`
  - [x] Tool `Error` → `ToolExecuted { success: false }`
  - [x] Worldstate refresh → `WorldstateUpdated`
  - [x] Error de red → `Error`
  - [x] Fissura Steel Path / Sortie nueva → `CriticalAlert`
- [ ] **Pendiente:** spinner "pensando" → `AudioEvent::SpinnerTick` (el evento
      existe en `src/audio.rs` pero aún no se emite desde el event loop)
- [ ] **Pendiente:** volumen configurable en `config.toml` (`audio_volume = 0.5`;
      hoy el volumen está fijo en `0.5`)
- [ ] **Pendiente:** mute toggle con tecla `M` en TUI
- [ ] **No aplica:** copiar audios a `data/audio/` — los archivos se leen
      directamente de `Audio/` del Fansite Kit resuelto con `audio_dir()`

---

## Licencia

El **Warframe Fansite Kit** es provisto por **Digital Extremes** para uso en fansites y proyectos comunitarios no comerciales. Ver términos en el kit original (Box link: `https://digitalextremesltd.app.box.com/s/vh2u41yhdlgp3girffucbynadoi33173`).

> **Nota:** No redistribuir los archivos de audio fuera del proyecto. El binario final debe incluirlos como recursos internos o cargarlos desde la ruta local del usuario.
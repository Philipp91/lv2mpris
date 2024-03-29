use std::sync::mpsc;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::Duration;

use lv2::prelude::*;
use mpris::{DBusError, Player, PlayerFinder};
use wmidi::{MidiMessage, U14};

// First is most preferred.
const PREFERRED_PLAYERS: &'static [&'static str] = &["Audacious", "Spotify", "Google Chrome"];

fn get_player_score(player: &Player) -> usize {
    PREFERRED_PLAYERS.iter().position(|p| *p == player.identity()).unwrap_or(usize::MAX)
}

fn find_player() -> Option<Player> {
    let finder = PlayerFinder::new().expect("Could not connect to D-Bus");
    let players = finder.find_all().expect("Could not list players");
    players.into_iter().min_by_key(|p| get_player_score(p))
}

const U14_MIDDLE: u16 = 1 << 13;
const U14_MAX: u16 = (1 << 14) - 1;

#[derive(PortCollection)]
struct Ports {
    #[allow(dead_code)]
    // These must match the indices in lv2mpris.
    midi_in: InputPort<AtomPort>,
    control_play: InputPort<Control>,
    control_pause: InputPort<Control>,
    control_play_pause: InputPort<Control>,
    control_stop: InputPort<Control>,
    control_prev: InputPort<Control>,
    control_next: InputPort<Control>,
    control_rewind: InputPort<Control>,
    control_forward: InputPort<Control>,
    control_shuffle: InputPort<Control>,
    control_lower_volume: InputPort<Control>,
    control_raise_volume: InputPort<Control>,
    control_mute: InputPort<Control>,
    control_raise: InputPort<Control>,
    volume: InputPort<Control>,
    enable_pitch_to_seek: InputPort<Control>,
}

type SimpleAction = fn(&Player) -> Result<(), DBusError>;

enum Action {
    Simple(SimpleAction),
    Seek(i64), // offset_in_microseconds
}

impl Action {
    fn run(&self, player: &Player) -> Option<()> {
        match self {
            Action::Simple(action) => (action)(player).ok(),
            Action::Seek(offset_in_microseconds) => player.seek(*offset_in_microseconds).ok(),
        }
    }
}

struct KeyMapping {
    input_port: fn(&Ports) -> &InputPort<Control>,
    last_state: bool,
    action: SimpleAction,
}

fn make_key_mapping(input_port: fn(&Ports) -> &InputPort<Control>,
                    action: SimpleAction) -> KeyMapping {
    KeyMapping { input_port, last_state: false, action }
}

const MIN_PITCH: u16 = U14_MIDDLE / 4; // We assume that quickly tapping the wheel bends it by at most a quarter.
const SEEK_STEP_DURATION: &'static Duration = &Duration::from_secs(5); // For a single step with a button.
const SEEK_PITCH_STEP_DURATION: i64 = Duration::from_secs(2).as_micros() as i64; // For a single tap of the pitch wheel.
const SEEK_PITCH_DURATION: &'static Duration = &Duration::from_secs(30); // Bending the pitch wheel all the way.
const SEEK_PITCH_HOLD_DURATION: i64 = Duration::from_secs(1).as_micros() as i64; // When holding the pitch wheel all the way.

fn default_key_mappings() -> Vec<KeyMapping> {
    vec![
        make_key_mapping(|ports: &Ports| &ports.control_play, |player: &Player| player.play()),
        make_key_mapping(|ports: &Ports| &ports.control_pause, |player: &Player| player.pause()),
        make_key_mapping(|ports: &Ports| &ports.control_play_pause, |player: &Player| player.play_pause()),
        make_key_mapping(|ports: &Ports| &ports.control_stop, |player: &Player| player.stop()),
        make_key_mapping(|ports: &Ports| &ports.control_prev, |player: &Player| player.previous()),
        make_key_mapping(|ports: &Ports| &ports.control_next, |player: &Player| player.next()),
        make_key_mapping(|ports: &Ports| &ports.control_rewind, |player: &Player| player.seek_backwards(SEEK_STEP_DURATION)),
        make_key_mapping(|ports: &Ports| &ports.control_forward, |player: &Player| player.seek_forwards(SEEK_STEP_DURATION)),
        make_key_mapping(|ports: &Ports| &ports.control_shuffle, |player: &Player| {
            player.set_shuffle(player.get_shuffle().unwrap_or(false))
        }),
        make_key_mapping(|ports: &Ports| &ports.control_lower_volume, |player: &Player| {
            player.set_volume(player.get_volume().unwrap_or(0.0) - 0.05)
        }),
        make_key_mapping(|ports: &Ports| &ports.control_raise_volume, |player: &Player| {
            player.set_volume(player.get_volume().unwrap_or(0.0) + 0.05)
        }),
        make_key_mapping(|ports: &Ports| &ports.control_mute, |player: &Player| player.set_volume(0.0)),
        make_key_mapping(|ports: &Ports| &ports.control_raise, |player: &Player| player.raise()),
    ]
}

#[derive(FeatureCollection)]
pub struct Features<'a> {
    map: LV2Map<'a>,
}

#[derive(URIDCollection)]
pub struct URIDs {
    atom: AtomURIDCollection,
    midi: MidiURIDCollection,
    unit: UnitURIDCollection,
}

#[uri("https://philippkeck.de/lv2/lv2mpris")]
struct LV2MPRIS {
    urids: URIDs,
    last_volume: f32,
    key_mappings: Vec<KeyMapping>,
    pitch_value: u16,
    work_transmitter: SyncSender<Action>,
}

impl Plugin for LV2MPRIS {
    type Ports = Ports;
    type InitFeatures = Features<'static>;
    type AudioFeatures = ();

    fn new(_plugin_info: &PluginInfo, features: &mut Features<'static>) -> Option<Self> {
        let (transmitter, receiver) = mpsc::sync_channel::<Action>(10);
        thread::spawn(move || {
            for action in receiver { // This loop ends when the last transmitter is deleted.
                if let Some(player) = find_player() {
                    action.run(&player);
                }
            }
        });
        Some(Self {
            urids: features.map.populate_collection()?,
            last_volume: f32::MIN,
            key_mappings: default_key_mappings(),
            pitch_value: U14_MIDDLE,
            work_transmitter: transmitter,
        })
    }

    fn run(&mut self, ports: &mut Ports, _: &mut (), _: u32) {
        self.run_key_mappings(ports);

        self.run_volume_update(ports);

        if *ports.enable_pitch_to_seek > 0.5 {
            self.run_pitch_bend_to_seek(ports);
        }
    }
}

impl LV2MPRIS {
    // Pass on volume changes.
    fn run_volume_update(&mut self, ports: &mut Ports) {
        if self.last_volume == f32::MIN { // Init
            self.last_volume = *ports.volume;
        } else if self.last_volume != *ports.volume { // Update
            self.last_volume = *ports.volume;
            if let Some(player) = find_player() {
                player.set_volume(self.last_volume as f64).ok();
            }
        }
    }

    // Map boolean control inputs to simple keypresses when they flip from 0 to 1.
    fn run_key_mappings(&mut self, ports: &Ports) {
        for key_mapping in &mut self.key_mappings {
            if **((key_mapping.input_port)(ports)) > 0.5 {
                if !key_mapping.last_state {
                    key_mapping.last_state = true;
                    self.work_transmitter.send(Action::Simple(key_mapping.action)).ok();
                }
            } else {
                if key_mapping.last_state {
                    key_mapping.last_state = false;
                }
            }
        }
    }

    fn run_pitch_bend_to_seek(&mut self, ports: &Ports) {
        let input_sequence = ports.midi_in.read(self.urids.atom.sequence, self.urids.unit.beat).unwrap();
        for (_, atom) in input_sequence {
            let value = atom.read(self.urids.midi.wmidi, ());
            if let Some(MidiMessage::PitchBendChange(_, pitch_bend)) = value {
                let new_pitch = U14::data_to_slice(&[pitch_bend])[0];
                self.on_pitch_change(self.pitch_value, new_pitch);
                self.pitch_value = new_pitch;
                return;
            }
        }

        // Keep seeking at a certain speed if
        if self.pitch_value == 0 {
            self.work_transmitter.send(Action::Seek(-SEEK_PITCH_HOLD_DURATION)).ok();
        } else if self.pitch_value == U14_MAX {
            self.work_transmitter.send(Action::Seek(SEEK_PITCH_HOLD_DURATION)).ok();
        }
    }

    // Returns microseconds
    fn pitch_to_seek_offset(pitch: f64) -> i64 {
        const MIDDLE_PITCH: f64 = U14_MIDDLE as f64;
        let fraction = (pitch - MIDDLE_PITCH) / MIDDLE_PITCH;
        return (SEEK_PITCH_DURATION.as_micros() as f64 * fraction * fraction * fraction) as i64;
    }

    fn on_pitch_change(&mut self, old_pitch: u16, new_pitch: u16) {
        if new_pitch < U14_MIDDLE && new_pitch > U14_MIDDLE - MIN_PITCH { // Do a single rewind step ...
            if old_pitch >= U14_MIDDLE { // ... but only if we just freshly entered the MIN_PITCH area.
                self.work_transmitter.send(Action::Seek(-SEEK_PITCH_STEP_DURATION)).ok();
            }
            return;
        } else if new_pitch > U14_MIDDLE && new_pitch < U14_MIDDLE + MIN_PITCH { // Do a single forward step ...
            if old_pitch <= U14_MIDDLE { // ... but only if we just freshly entered the MIN_PITCH area.
                self.work_transmitter.send(Action::Seek(SEEK_PITCH_STEP_DURATION)).ok();
            }
            return;
        }

        // Now we're beyond the MIN_PITCH area. Only seek if the wheel was moved further than it was before, i.e.
        // do nothing when the wheel is being released.
        if (new_pitch > U14_MIDDLE && new_pitch > old_pitch) ||
            (new_pitch < U14_MIDDLE && new_pitch < old_pitch) {
            let old_offset = LV2MPRIS::pitch_to_seek_offset(old_pitch as f64);
            let new_offset = LV2MPRIS::pitch_to_seek_offset(new_pitch as f64);
            let corrected_old_offset = if old_offset.signum() == new_offset.signum() { old_offset } else { 0 };
            self.work_transmitter.send(Action::Seek(new_offset - corrected_old_offset)).ok();
        }
    }
}

lv2_descriptors!(LV2MPRIS);

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_test() {
        if let Some(player) = find_player() {
            println!("Player: {}", player.identity());
            println!("Shuffle: {}", player.checked_set_shuffle(false).unwrap());
        }
        println!("DONE");
    }
}

# LV2 MPRIS

This plugin receives LV2 control messages (which you can MIDI-learn in Carla,
for example) and redirects them as control commands to the active MPRIS player.

The plugin is designed to work with buttons (presumably somewhere on your
musical instrument or some other hardware) that flip the control input to 1 (the
maximum value) when pressed down, and back to 0 (the minimum) when released.
A keypress is only simulated when the input button's state switches from 0 to 1.

In addition to simple commands like "play" and "pause", the plugin also supports
setting the volume directly from a linear input control (like a slider or knob)
as well as seeking (rewinding or fast-forwarding) with the pitch-bend wheel.

## For users

### Installation

If you happen to have a x64 linux machine, just download the zipped release from
GitHub and unpack it to `~/.lv2` or `/usr/local/lib/lv2` or wherever else your
plugin host discovers LV2 plugins.

Otherwise, clone this repository, install dependencies (see below) and run
`./build.sh` to build from source.

### Configuration

Add the plugin to your LV2 host (e.g. [Carla](https://github.com/falkTX/Carla)).
In the plugin configuration you should see input "parameters" or "controls"
(called "Play", "Pause", ...). These map to the respective multimedia keys on
your keyboard (even if your physical keyboard does not have those keys, see also
[this list](https://wiki.linuxquestions.org/wiki/XF86_keyboard_symbols)). Pick
the ones you use and assign your buttons to them, e.g. with a MIDI learn
function. If you want to use the pitch-bend wheel for seeking, just flip the
respective option to 1 manually.

## For developers

PRs for fixes, new functionality and code style improvements are welcome.

### Dependencies

```bash
# Required libraries.
sudo apt-get install lv2-dev libdbus-glib-1-dev

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build

To produce the deployable `lv2mpris.lv2` output directory.

```bash
./build.sh
```

### Manual test

To rebuild and try the plugin in Carla:

```bash
./develop.sh
```

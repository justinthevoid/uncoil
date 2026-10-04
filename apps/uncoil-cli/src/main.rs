//! `uncoil` — command line for the uncoil daemon. Every command goes through uncoild's control pipe; the CLI
//! never opens a device itself (the daemon owns them and streams lighting).

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use uncoil_core::config::Status;
use uncoil_core::device::Support;
use uncoil_core::features::dial::{DialMode, DialState};
use uncoil_core::features::hw_effect::{parse_color, Direction, HwEffect, Storage, DEFAULT_WAVE_SPEED};
use uncoil_core::features::keymap::{Function, KeymapFile, Layer};
use uncoil_core::features::oled::OledState;
use uncoil_core::features::parse_u8;
use uncoil_core::features::performance::{Dpi, DpiStages, PerformanceState};
use uncoil_core::features::power::PowerState;
use uncoil_core::features::profile::ProfileInfo;
use uncoil_core::ipc::{self, *};

const HELP: &str = "\
uncoil — talk to the uncoil daemon (uncoild)

usage: uncoil [--json] [--pipe NAME] <command>

  status                               daemon status and its own footprint
  devices                              connected devices
  caps [DEVICE] [--probe]              what a device supports (--probe: ask the device too)

  keymap get DEVICE KEY [--layer L]    one key's mapping (L: normal | fn; default normal)
  keymap dump DEVICE [--layer L]       every key on a layer
  keymap set DEVICE KEY MAPPING… --write [--layer L]
                                       write a mapping to the device, e.g.
                                         uncoil keymap set keyboard P key PRINT_SCREEN --layer fn --write
  keymap reset DEVICE KEY --write [--layer L]
                                       restore what was there before uncoil first wrote it
                                       (else the key's default from the device definition)
  keymap export DEVICE FILE [--layer L|both]
                                       back up a layer (default: both) to a TOML file
  keymap import DEVICE FILE [--write]  show what differs from FILE; with --write, apply it

  profile list DEVICE                  onboard profiles
  dial get DEVICE                      active command-dial mode
  dial set DEVICE MODE --write [--enabled A,B,…]
                                       modes: VOLUME TRACK_SELECTOR OLED_BRIGHTNESS LIGHTNING_BRIGHTNESS
                                       SWITCH_APPS ZOOM TRACK_JOGGING SCROLL_VERTICAL SCROLL_HORIZONTAL
  oled get DEVICE                      OLED display settings
  oled set DEVICE --brightness N --write
  effect hw DEVICE EFFECT [--color C]… [--direction left|right] [--speed N] [--duration N]
                                       firmware effect: off static breathing spectrum wave wheel
                                       reactive starlight. Shown until the config changes or
                                       `effect software`; add --onboard --write to save it in the device
  effect software DEVICE               back to the configured software effect

  dpi DEVICE                           current DPI, DPI stages and poll rate
  dpi DEVICE N[xM]                     set the DPI now, like the DPI button (not stored; no --write)
  dpi DEVICE --stages A,B,C… [--active N] --write
                                       store the DPI stages (each N or NxM; --active is 1-based)
  poll DEVICE [HZ --write]             show or store the poll rate
  power DEVICE                         battery, charging, sleep timer, low-battery warning
  power DEVICE [--idle SECONDS] [--low-battery PERCENT] --write
                                       store the sleep timer (60-900 s) / warning level (5-25 %)
  check DEVICE                         run the read-only checks now (experimental devices and
                                       features not yet confirmed: their writes wait for these)

DEVICE: an id, a kind (keyboard, mouse, mat) or part of the name (basilisk).
KEY: a key name (P, F9, PAGE_UP, \"Page Up\") or #id.
MAPPING: off | key NAME [+lctrl +lshift …] | button N | razer N | power 0x82 | media B B | profile N
         | dpi B… | turbo-button BUTTON MS | raw FN B…
Every command that changes the device's onboard memory needs --write and prints before/after.
On experimental devices those writes also wait for the matching read-only check (`uncoil check`).
";

#[derive(Debug, Default)]
struct Opts {
    json: bool,
    write: bool,
    probe: bool,
    onboard: bool,
    pipe: Option<String>,
    layer: Option<String>,
    profile: Option<u8>,
    brightness: Option<u8>,
    enabled: Option<String>,
    colors: Vec<String>,
    direction: Option<String>,
    speed: Option<u8>,
    duration: Option<u8>,
    stages: Option<String>,
    active: Option<u8>,
    idle: Option<u16>,
    low_battery: Option<u8>,
}

#[derive(Debug, PartialEq)]
enum Action {
    Help,
    /// A daemon command, with the device query if any.
    Call(Option<String>, Command),
    Export {
        device: String,
        file: String,
        layers: Vec<Layer>,
        profile: u8,
    },
    Import {
        device: String,
        file: String,
        write: bool,
    },
}

fn parse(argv: &[String]) -> Result<(Opts, Action)> {
    let mut o = Opts::default();
    let mut pos: Vec<String> = vec![];
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or_else(|| anyhow!("{name} needs a value"));
        match a.as_str() {
            "--json" => o.json = true,
            "--write" => o.write = true,
            "--probe" => o.probe = true,
            "--onboard" => o.onboard = true,
            "--pipe" => o.pipe = Some(val(a)?),
            "--layer" => o.layer = Some(val(a)?),
            "--fn" => o.layer = Some("fn".into()),
            "--profile" => o.profile = Some(byte(&val(a)?)?),
            "--brightness" => o.brightness = Some(byte(&val(a)?)?),
            "--enabled" => o.enabled = Some(val(a)?),
            "--color" | "--colour" => o.colors.push(val(a)?),
            "--direction" => o.direction = Some(val(a)?),
            "--speed" => o.speed = Some(byte(&val(a)?)?),
            "--duration" => o.duration = Some(byte(&val(a)?)?),
            "--stages" => o.stages = Some(val(a)?),
            "--active" => o.active = Some(byte(&val(a)?)?),
            "--idle" => o.idle = Some(number(&val(a)?)?),
            "--low-battery" => o.low_battery = Some(byte(val(a)?.trim_end_matches('%'))?),
            "-h" | "--help" | "help" => return Ok((o, Action::Help)),
            s if s.starts_with("--") => bail!("unknown option {s}"),
            _ => pos.push(a.clone()),
        }
    }
    let layer = match &o.layer {
        Some(l) => Layer::parse(l).ok_or_else(|| anyhow!("unknown layer `{l}` (normal or fn)"))?,
        None => Layer::Normal,
    };
    let profile = o.profile.unwrap_or(1);
    let p: Vec<&str> = pos.iter().map(String::as_str).collect();
    let dev = |i: usize| p.get(i).map(|s| s.to_string()).ok_or_else(|| anyhow!("missing DEVICE (see `uncoil help`)"));
    let need = |i: usize, what: &str| p.get(i).map(|s| s.to_string()).ok_or_else(|| anyhow!("missing {what}"));
    let call = |d: String, c: Command| Action::Call(Some(d), c);
    let action = match p.as_slice() {
        [] => Action::Help,
        ["status"] => Action::Call(None, Command::Status),
        ["devices"] => Action::Call(None, Command::Devices),
        ["caps" | "capabilities", rest @ ..] => Action::Call(
            rest.first().map(|s| s.to_string()),
            Command::Capabilities(CapabilitiesArgs { probe: o.probe }),
        ),
        ["keymap", "get", ..] => call(dev(2)?, Command::KeymapGet(KeyArgs { key: need(3, "KEY")?, layer, profile })),
        ["keymap", "dump", ..] => call(dev(2)?, Command::KeymapDump(LayerArgs { layer, profile })),
        ["keymap", "set", _, _, spec @ ..] if !spec.is_empty() => {
            let function = Function::parse_spec(&spec.join(" "))?;
            call(
                dev(2)?,
                Command::KeymapSet(KeySetArgs { key: need(3, "KEY")?, layer, profile, function, write: o.write }),
            )
        }
        ["keymap", "set", ..] => bail!("usage: uncoil keymap set DEVICE KEY MAPPING… --write"),
        ["keymap", "reset", ..] => {
            call(dev(2)?, Command::KeymapReset(KeyResetArgs { key: need(3, "KEY")?, layer, profile, write: o.write }))
        }
        ["keymap", "export", ..] => {
            let layers = match o.layer.as_deref() {
                None | Some("both" | "all") => vec![Layer::Normal, Layer::Hypershift],
                Some(_) => vec![layer],
            };
            Action::Export { device: dev(2)?, file: need(3, "FILE")?, layers, profile }
        }
        ["keymap", "import", ..] => Action::Import { device: dev(2)?, file: need(3, "FILE")?, write: o.write },
        ["profile" | "profiles", "list", ..] | ["profiles", ..] => {
            call(dev(2).or_else(|_| dev(1))?, Command::ProfileList)
        }
        ["dial", "get", ..] => call(dev(2)?, Command::DialGet(ProfileArgs { profile })),
        ["dial", "set", _, mode, ..] => {
            let mode = DialMode::parse(mode).ok_or_else(|| anyhow!("unknown dial mode `{mode}`"))?;
            let enabled = match &o.enabled {
                Some(list) => Some(
                    list.split(',')
                        .map(|m| DialMode::parse(m.trim()).ok_or_else(|| anyhow!("unknown dial mode `{m}`")))
                        .collect::<Result<Vec<_>>>()?,
                ),
                None => None,
            };
            call(dev(2)?, Command::DialSet(DialSetArgs { mode, profile, enabled, write: o.write }))
        }
        ["oled", "get", ..] => call(dev(2)?, Command::OledGet),
        ["oled", "set", ..] => {
            call(dev(2)?, Command::OledSet(OledSetArgs { brightness: o.brightness, write: o.write }))
        }
        ["effect", "hw", _, name, ..] => {
            let effect = hw_effect(name, &o)?;
            let storage = if o.onboard { Storage::Onboard } else { Storage::Session };
            call(dev(2)?, Command::EffectHw(EffectHwArgs { effect, storage, write: o.write }))
        }
        ["effect", "software" | "sw", ..] => call(dev(2)?, Command::EffectSoftware),
        ["dpi", d, rest @ ..] => {
            let dpi = rest.first().map(|v| parse_dpi(v)).transpose()?;
            let stages = match &o.stages {
                Some(list) => {
                    let list = list.split(',').map(|v| parse_dpi(v.trim())).collect::<Result<Vec<_>>>()?;
                    Some(DpiStages { active: o.active.unwrap_or(1), list })
                }
                None if o.active.is_some() => bail!("--active goes with --stages"),
                None => None,
            };
            if dpi.is_none() && stages.is_none() {
                call(d.to_string(), Command::PerformanceGet)
            } else {
                call(
                    d.to_string(),
                    Command::PerformanceSet(PerformanceSetArgs { dpi, stages, poll_hz: None, write: o.write }),
                )
            }
        }
        ["poll", d] => call(d.to_string(), Command::PerformanceGet),
        ["poll", d, hz, ..] => {
            let hz = number(hz.trim_end_matches("Hz").trim_end_matches("hz"))?;
            call(
                d.to_string(),
                Command::PerformanceSet(PerformanceSetArgs { poll_hz: Some(hz), write: o.write, ..Default::default() }),
            )
        }
        ["power", d, ..] if o.idle.is_none() && o.low_battery.is_none() => call(d.to_string(), Command::PowerGet),
        ["power", d, ..] => call(
            d.to_string(),
            Command::PowerSet(PowerSetArgs { idle_s: o.idle, low_battery_pct: o.low_battery, write: o.write }),
        ),
        ["check" | "checks", d, ..] => call(d.to_string(), Command::CheckRun),
        other => bail!("unknown command `{}` (see `uncoil help`)", other.join(" ")),
    };
    Ok((o, action))
}

fn byte(s: &str) -> Result<u8> {
    parse_u8(s).ok_or_else(|| anyhow!("not a number 0-255: {s}"))
}

fn number(s: &str) -> Result<u16> {
    s.trim().parse().map_err(|_| anyhow!("not a number 0-65535: {s}"))
}

/// `800` or `800x600`.
fn parse_dpi(s: &str) -> Result<Dpi> {
    let (x, y) = s.split_once(['x', 'X']).unwrap_or((s, s));
    Ok(Dpi { x: number(x)?, y: number(y)? })
}

fn check_word(s: CheckState) -> &'static str {
    match s {
        CheckState::Passed => "passed",
        CheckState::Failed => "FAILED",
        CheckState::Untested => "not run yet",
        CheckState::NotNeeded => "not needed (confirmed device)",
    }
}

fn print_performance(s: &PerformanceState) {
    let opt = |v: Option<u16>| v.map_or("?".to_string(), |x| x.to_string());
    if s.dpi_max.is_some() {
        let dpi = s.dpi.map_or("? (no answer)".to_string(), |d| d.to_string());
        println!("DPI         {dpi}  (range {}-{})", opt(s.dpi_min), opt(s.dpi_max));
    }
    if let Some(st) = &s.stages {
        let list: Vec<String> = st
            .list
            .iter()
            .enumerate()
            .map(|(i, d)| if i + 1 == st.active as usize { format!("[{d}]") } else { d.to_string() })
            .collect();
        println!("stages      {}  (up to {})", list.join(" "), s.stages_max);
    } else if s.stages_max > 0 {
        println!("stages      ? (no answer)");
    }
    if !s.poll_rates.is_empty() {
        let rates: Vec<String> = s.poll_rates.iter().map(u16::to_string).collect();
        println!("poll rate   {} Hz  (offers {} Hz)", opt(s.poll_hz), rates.join(", "));
    }
}

fn print_power(s: &PowerState) {
    if let Some(b) = s.battery_pct {
        println!("battery       {b}%{}", if s.charging == Some(true) { ", charging" } else { "" });
    }
    if s.idle_range.is_some() {
        println!("sleep after   {} s", s.idle_s.map_or("?".into(), |v| v.to_string()));
    }
    if s.low_battery_range.is_some() {
        println!("low battery   {}%", s.low_battery_pct.map_or("?".into(), |v| v.to_string()));
    }
}

fn print_verified<T>(r: &WriteResult<T>) {
    if r.unchanged {
        println!("(already set; nothing written)");
    } else if r.verified {
        println!("written to the device and read back");
    } else {
        println!("WARNING: read-back differs from what was written");
    }
}

fn hw_effect(name: &str, o: &Opts) -> Result<HwEffect> {
    let colors = o
        .colors
        .iter()
        .map(|c| parse_color(c).ok_or_else(|| anyhow!("bad colour `{c}` (#rrggbb or r,g,b)")))
        .collect::<Result<Vec<_>>>()?;
    let first = || colors.first().copied().ok_or_else(|| anyhow!("{name} needs --color"));
    let direction = match o.direction.as_deref() {
        None => Direction::default(),
        Some("left") => Direction::Left,
        Some("right") => Direction::Right,
        Some(d) => bail!("direction is left or right, not {d}"),
    };
    let speed = o.speed.unwrap_or(DEFAULT_WAVE_SPEED);
    let duration = o.duration.unwrap_or(2);
    Ok(match name {
        "off" => HwEffect::Off,
        "static" => HwEffect::Static { color: first()? },
        "breathing" | "breath" => HwEffect::Breathing { colors },
        "spectrum" => HwEffect::Spectrum,
        "wave" => HwEffect::Wave { direction, speed },
        "wheel" => HwEffect::Wheel { direction, speed },
        "reactive" => HwEffect::Reactive { color: first()?, duration },
        "starlight" => HwEffect::Starlight { colors, duration },
        other => bail!("unknown effect `{other}`"),
    })
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&argv) {
        eprintln!("uncoil: {e:#}");
        std::process::exit(1);
    }
}

fn connect(o: &Opts) -> Result<Client> {
    let name = o.pipe.clone().or_else(|| std::env::var("UNCOIL_PIPE").ok()).unwrap_or_else(|| ipc::PIPE_NAME.into());
    Client::connect_to(&name)
}

fn run(argv: &[String]) -> Result<()> {
    let (o, action) = parse(argv)?;
    match action {
        Action::Help => print!("{HELP}"),
        Action::Call(device, cmd) => {
            let mut c = connect(&o)?;
            let v: Value = c.call(device.as_deref(), &cmd)?.into_result()?;
            if o.json {
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                show(&cmd, v)?;
            }
        }
        Action::Export { device, file, layers, profile } => {
            let mut c = connect(&o)?;
            let id = resolve_id(&mut c, &device).unwrap_or_else(|| device.clone());
            let mut out = KeymapFile { device: id, profile, ..Default::default() };
            for layer in layers {
                let rows: Vec<KeyMapping> =
                    c.call(Some(&device), &Command::KeymapDump(LayerArgs { layer, profile }))?.into_result()?;
                *out.layer_mut(layer) = rows.into_iter().map(|r| (r.name, r.function.to_string())).collect();
            }
            let n = out.normal.len() + out.hypershift.len();
            std::fs::write(&file, out.to_toml()?).with_context(|| format!("write {file}"))?;
            if o.json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("saved {n} mappings of {} to {file}", out.device);
            }
        }
        Action::Import { device, file, write } => import(&o, &device, &file, write)?,
    }
    Ok(())
}

fn resolve_id(c: &mut Client, query: &str) -> Option<String> {
    let caps: Capabilities =
        c.call(Some(query), &Command::Capabilities(CapabilitiesArgs::default())).ok()?.into_result().ok()?;
    Some(caps.id)
}

fn import(o: &Opts, device: &str, file: &str, write: bool) -> Result<()> {
    let src = std::fs::read_to_string(file).with_context(|| format!("read {file}"))?;
    let f = KeymapFile::from_toml(&src)?;
    let mut c = connect(o)?;
    if let Some(id) = resolve_id(&mut c, device) {
        if id != f.device {
            bail!("{file} is a backup of {}, not {id}", f.device);
        }
    }
    // what changes
    let mut changes: Vec<(Layer, String, Function, KeyMapping)> = vec![];
    for layer in [Layer::Normal, Layer::Hypershift] {
        let wanted: &BTreeMap<String, String> = f.layer(layer);
        if wanted.is_empty() {
            continue;
        }
        let rows: Vec<KeyMapping> =
            c.call(Some(device), &Command::KeymapDump(LayerArgs { layer, profile: f.profile }))?.into_result()?;
        for (key, spec) in wanted {
            let target = Function::parse_spec(spec).with_context(|| format!("{file}: {key} = \"{spec}\""))?;
            let cur = rows.iter().find(|r| &r.name == key).ok_or_else(|| anyhow!("{file}: unknown key {key}"))?;
            if cur.function != target {
                changes.push((layer, key.clone(), target, cur.clone()));
            }
        }
    }
    if changes.is_empty() {
        println!("{device} already matches {file}");
        return Ok(());
    }
    for (layer, key, target, cur) in &changes {
        println!("  {:<10} {:<14} {:<28} -> {target}", layer.as_str(), key, cur.function.to_string());
    }
    if !write {
        println!("{} change(s); nothing written. Repeat with --write to apply.", changes.len());
        return Ok(());
    }
    for (layer, key, function, _) in changes {
        let r: WriteResult<KeyMapping> = c
            .call(
                Some(device),
                &Command::KeymapSet(KeySetArgs { key: key.clone(), layer, profile: f.profile, function, write: true }),
            )?
            .into_result()?;
        print_write(&key, layer, &r);
    }
    Ok(())
}

fn print_write(key: &str, layer: Layer, r: &WriteResult<KeyMapping>) {
    if r.unchanged {
        println!("{key} ({}) already `{}`; nothing written", layer.as_str(), r.after.function);
        return;
    }
    println!("{key} ({}, profile {}):", layer.as_str(), r.after.profile);
    println!("  before: {:<28} {}", r.before.function.to_string(), r.before.description);
    println!("  after:  {:<28} {}", r.after.function.to_string(), r.after.description);
    println!(
        "  {}",
        if r.verified {
            "written to the device and read back"
        } else {
            "WARNING: read-back differs from what was written"
        }
    );
}

fn show(cmd: &Command, v: Value) -> Result<()> {
    match cmd {
        Command::Status => {
            let s: Status = serde_json::from_value(v)?;
            println!("uncoild {} (pid {}), display {}, level {:.2}", s.version, s.pid, s.display, s.level);
            for u in &s.unknown_devices {
                println!("  a Razer device uncoil doesn't know yet (product ID 0x{:04X})", u.product_id);
            }
            println!(
                "footprint: {:.1} MB private memory, {:.2}% of one core, {:.2} MB executable",
                s.memory_bytes as f64 / 1e6,
                s.cpu_percent,
                s.exe_bytes as f64 / 1e6
            );
            for d in s.devices {
                println!("  {:<34} {:>5.1} fps  {} busy retries, {} errors", d.name, d.fps, d.busy_retries, d.errors);
            }
        }
        Command::Devices => {
            let devs: Vec<DeviceInfo> = serde_json::from_value(v)?;
            if devs.is_empty() {
                println!("no devices connected");
            }
            for d in devs {
                let f: Vec<&str> = d.features.iter().map(|f| f.as_str()).collect();
                let hw = d.hw_effect.map(|e| format!("  [firmware effect: {}]", e.name())).unwrap_or_default();
                let exp = if d.support == Support::Experimental { "  [experimental]" } else { "" };
                println!(
                    "{:<34} {:<32} {:04X} {:<7} {}{hw}{exp}",
                    d.name,
                    d.id,
                    d.product_id,
                    d.connection,
                    f.join(" ")
                );
            }
        }
        Command::Capabilities(_) => {
            let list: Vec<Capabilities> =
                if v.is_array() { serde_json::from_value(v)? } else { vec![serde_json::from_value(v)?] };
            for c in list {
                let f: Vec<&str> = c.features.iter().map(|f| f.as_str()).collect();
                println!("{} ({}){}", c.name, c.id, if c.connected { "" } else { " — not connected" });
                println!("  features:    {}", f.join(", "));
                if c.support == Support::Experimental {
                    println!(
                        "  support:     experimental (set up from OpenRazer and OpenRGB data; nobody has confirmed it)"
                    );
                }
                if !c.unverified.is_empty() {
                    let u: Vec<&str> = c.unverified.iter().map(|f| f.as_str()).collect();
                    println!("  unconfirmed: {}", u.join(", "));
                }
                for ch in c.checks.iter().filter(|ch| ch.state != CheckState::NotNeeded) {
                    println!("  check:       {:<11} {}", ch.feature.as_str(), check_word(ch.state));
                }
                if !c.hw_effects.is_empty() {
                    println!("  hw effects:  {}", c.hw_effects.join(", "));
                }
                if !c.keys.is_empty() {
                    let layers: Vec<&str> = c.keymap_layers.iter().map(|l| l.as_str()).collect();
                    let names: Vec<&str> = c.keys.iter().map(|k| k.name.as_str()).collect();
                    println!("  key map:     {} keys, layers {}", c.keys.len(), layers.join(" + "));
                    println!("               {}", names.join(" "));
                }
                if !c.dial_modes.is_empty() {
                    let m: Vec<&str> = c.dial_modes.iter().map(|m| m.name()).collect();
                    println!("  dial modes:  {}", m.join(" "));
                }
                if let Some(p) = c.probed {
                    for r in p.regions {
                        println!("  region:      led {} ({}x{})", r.led, r.rows, r.cols);
                    }
                    for (led, fx) in p.effects {
                        println!("  firmware:    led {led}: {}", fx.join(", "));
                    }
                }
            }
        }
        Command::KeymapGet(_) => {
            let m: KeyMapping = serde_json::from_value(v)?;
            println!("{} ({}, profile {}): {}  — {}", m.name, m.layer.as_str(), m.profile, m.function, m.description);
        }
        Command::KeymapDump(a) => {
            let rows: Vec<KeyMapping> = serde_json::from_value(v)?;
            println!("{} layer, profile {}:", a.layer.as_str(), a.profile);
            for m in rows {
                println!("  {:<24} {:<28} {}", m.name, m.function.to_string(), m.description);
            }
        }
        Command::KeymapSet(a) => print_write(&a.key, a.layer, &serde_json::from_value(v)?),
        Command::KeymapReset(a) => print_write(&a.key, a.layer, &serde_json::from_value(v)?),
        Command::ProfileList => {
            let p: ProfileInfo = serde_json::from_value(v)?;
            let ids: Vec<String> = p.ids.iter().map(u8::to_string).collect();
            println!("{} of {} onboard profiles in use: {}", p.count, p.max, ids.join(", "));
            if let Some(a) = p.active {
                println!("active (05/84, unconfirmed meaning): {a}");
            }
        }
        Command::DialGet(_) => {
            let d: DialState = serde_json::from_value(v)?;
            let name = d.mode.map(|m| m.name().to_string()).unwrap_or_else(|| format!("unknown mode {}", d.mode_id));
            println!("profile {}: {name}", d.profile);
            if let Some(m) = d.mode {
                let [cw, press, ccw] = m.synapse_actions();
                println!("  (in Synapse: turn right {cw}, press {press}, turn left {ccw})");
            }
        }
        Command::DialSet(_) => {
            let r: WriteResult<DialState> = serde_json::from_value(v)?;
            let n = |d: &DialState| d.mode.map(|m| m.name().to_string()).unwrap_or_else(|| d.mode_id.to_string());
            println!("dial mode: {} -> {}{}", n(&r.before), n(&r.after), if r.unchanged { " (unchanged)" } else { "" });
            if !r.verified {
                println!("WARNING: read-back differs from what was written");
            }
        }
        Command::OledGet => {
            let s: OledState = serde_json::from_value(v)?;
            let o = |x: Option<u8>| x.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
            println!("brightness        {}%", o(s.brightness));
            println!("home screen       {} {}", s.home_screen.unwrap_or_default(), o(s.home_screen_index));
            println!("active item       {}", s.active_item.unwrap_or_default());
            println!("dim after         {} min", o(s.time_to_dim_minutes));
            println!("home after        code {} (1 = 5 s)", o(s.time_to_home));
            println!("low battery warn  {}%", o(s.low_battery_warning_percent));
            if let Some(a) = s.animations_enabled {
                println!("animations on     {}/{}", a.iter().filter(|x| **x).count(), a.len());
            }
        }
        Command::OledSet(_) => {
            let r: WriteResult<u8> = serde_json::from_value(v)?;
            println!(
                "OLED brightness: {}% -> {}%{}",
                r.before,
                r.after,
                if r.verified { "" } else { "  (WARNING: read-back differs)" }
            );
        }
        Command::EffectHw(a) => {
            println!(
                "firmware effect {} {}",
                a.effect.name(),
                if a.storage == Storage::Onboard {
                    "saved to the device"
                } else {
                    "shown (until the config changes or `uncoil effect software`)"
                }
            );
        }
        Command::EffectSoftware => println!("back to the software effect"),
        Command::CheckRun => {
            let list: Vec<FeatureCheck> = serde_json::from_value(v)?;
            for c in list {
                let detail = c.detail.map(|d| format!("  {d}")).unwrap_or_default();
                println!("{:<11} {}{detail}", c.feature.as_str(), check_word(c.state));
            }
        }
        Command::PerformanceGet => print_performance(&serde_json::from_value(v)?),
        Command::PerformanceSet(_) => {
            if v.get("before").is_some() {
                let r: WriteResult<PerformanceState> = serde_json::from_value(v)?;
                println!("before:");
                print_performance(&r.before);
                println!("after:");
                print_performance(&r.after);
                print_verified(&r);
            } else {
                print_performance(&serde_json::from_value(v)?);
            }
        }
        Command::PowerGet => print_power(&serde_json::from_value(v)?),
        Command::PowerSet(_) => {
            let r: WriteResult<PowerState> = serde_json::from_value(v)?;
            println!("before:");
            print_power(&r.before);
            println!("after:");
            print_power(&r.after);
            print_verified(&r);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<(Opts, Action)> {
        let argv: Vec<String> = s.split_whitespace().map(String::from).collect();
        parse(&argv)
    }

    #[test]
    fn keymap_set_parses_spec_and_requires_flag_in_daemon() {
        let (_, a) = p("keymap set keyboard P key PRINT_SCREEN --layer fn --write").unwrap();
        assert_eq!(
            a,
            Action::Call(
                Some("keyboard".into()),
                Command::KeymapSet(KeySetArgs {
                    key: "P".into(),
                    layer: Layer::Hypershift,
                    profile: 1,
                    function: Function::Key { modifiers: 0, usage: 0x46 },
                    write: true
                })
            )
        );
        let (_, a) = p("keymap set keyboard P key A +lctrl").unwrap();
        match a {
            Action::Call(_, Command::KeymapSet(k)) => {
                assert!(!k.write);
                assert_eq!(k.function, Function::Key { modifiers: 1, usage: 4 });
            }
            other => panic!("{other:?}"),
        }
        assert!(p("keymap set keyboard P").is_err());
        assert!(p("keymap set keyboard P key NOPE").is_err());
    }

    #[test]
    fn other_commands() {
        assert!(matches!(p("status").unwrap().1, Action::Call(None, Command::Status)));
        assert!(matches!(
            p("caps mouse --probe").unwrap().1,
            Action::Call(Some(_), Command::Capabilities(CapabilitiesArgs { probe: true }))
        ));
        assert!(matches!(
            p("keymap dump kb --layer fn").unwrap().1,
            Action::Call(_, Command::KeymapDump(LayerArgs { layer: Layer::Hypershift, .. }))
        ));
        assert!(matches!(
            p("dial set kb zoom --write").unwrap().1,
            Action::Call(_, Command::DialSet(DialSetArgs { mode: DialMode::Zoom, write: true, .. }))
        ));
        assert!(matches!(
            p("oled set kb --brightness 40").unwrap().1,
            Action::Call(_, Command::OledSet(OledSetArgs { brightness: Some(40), write: false }))
        ));
        assert!(matches!(p("profile list mouse").unwrap().1, Action::Call(_, Command::ProfileList)));
        assert!(matches!(
            p("keymap export kb backup.toml").unwrap().1,
            Action::Export { ref layers, .. } if layers.len() == 2
        ));
        assert!(
            matches!(p("keymap export kb b.toml --layer fn").unwrap().1, Action::Export { ref layers, .. } if layers == &[Layer::Hypershift])
        );
        assert!(matches!(p("keymap import kb b.toml --write").unwrap().1, Action::Import { write: true, .. }));
        assert!(matches!(p("").unwrap().1, Action::Help));
        assert!(p("frobnicate").is_err());
        assert!(p("keymap get kb P --layer sideways").is_err());
    }

    #[test]
    fn performance_power_and_checks() {
        assert!(matches!(p("dpi mouse").unwrap().1, Action::Call(_, Command::PerformanceGet)));
        match p("dpi mouse 800x600").unwrap().1 {
            Action::Call(_, Command::PerformanceSet(a)) => {
                assert_eq!(a.dpi, Some(Dpi { x: 800, y: 600 }));
                assert!(!a.write && a.stages.is_none());
            }
            other => panic!("{other:?}"),
        }
        match p("dpi mouse --stages 400,800,1600x1200 --active 2 --write").unwrap().1 {
            Action::Call(_, Command::PerformanceSet(a)) => {
                let s = a.stages.unwrap();
                assert_eq!((s.active, s.list.len(), s.list[2]), (2, 3, Dpi { x: 1600, y: 1200 }));
                assert!(a.write);
            }
            other => panic!("{other:?}"),
        }
        assert!(p("dpi mouse --active 2").is_err());
        assert!(p("dpi mouse fast").is_err());
        assert!(matches!(p("poll mouse").unwrap().1, Action::Call(_, Command::PerformanceGet)));
        assert!(matches!(
            p("poll mouse 500 --write").unwrap().1,
            Action::Call(_, Command::PerformanceSet(PerformanceSetArgs { poll_hz: Some(500), write: true, .. }))
        ));
        assert!(matches!(p("power mouse").unwrap().1, Action::Call(_, Command::PowerGet)));
        assert!(matches!(
            p("power mouse --idle 300 --low-battery 15% --write").unwrap().1,
            Action::Call(
                _,
                Command::PowerSet(PowerSetArgs { idle_s: Some(300), low_battery_pct: Some(15), write: true })
            )
        ));
        assert!(matches!(p("check deathadder").unwrap().1, Action::Call(Some(_), Command::CheckRun)));
    }

    #[test]
    fn effects() {
        let (_, a) = p("effect hw kb wave --direction left --speed 16").unwrap();
        match a {
            Action::Call(_, Command::EffectHw(e)) => {
                assert_eq!(e.effect, HwEffect::Wave { direction: Direction::Left, speed: 16 });
                assert_eq!(e.storage, Storage::Session);
            }
            other => panic!("{other:?}"),
        }
        let (_, a) = p("effect hw mat static --color #00ff80 --onboard --write").unwrap();
        assert!(matches!(
            a,
            Action::Call(_, Command::EffectHw(EffectHwArgs { storage: Storage::Onboard, write: true, .. }))
        ));
        assert!(p("effect hw kb static").is_err(), "static needs a colour");
        assert!(p("effect hw kb disco").is_err());
    }
}

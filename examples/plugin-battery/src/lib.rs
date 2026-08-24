//! `plugin-battery` — battery state as `#{…}` format variables.
//!
//! This is the shape of plugin the format ABI exists for. The usual way to put
//! battery state in a tmux status line is `#(battery.sh)`, which forks a shell
//! script on **every status interval, forever**. Here the reading is taken in
//! the server process, cached, and handed to the format engine directly:
//!
//! ```tmux
//! znative load path:examples/plugin-battery
//! set -g status-right '#{plugin_battery} | %H:%M'
//! ```
//!
//! Formats provided:
//!
//! | Format | Value |
//! | --- | --- |
//! | `#{plugin_battery}` | the reading with its prefix, e.g. `AC 87%` |
//! | `#{plugin_battery_pct}` | the percentage alone, e.g. `87` |
//! | `#{plugin_battery_state}` | `charging`, `discharging`, or `charged` |
//!
//! Configured with options, the way tmux plugins are:
//!
//! ```tmux
//! set -g @battery-charging-prefix    'AC '
//! set -g @battery-discharging-prefix 'BAT '
//! ```
//!
//! A format provider runs on **every redraw that mentions it**, so the reading
//! is cached for [`TTL`] and only the cache is consulted in between. Declining
//! (returning `None`) leaves the key to resolve the way it would have without
//! the plugin, which is what happens on a machine with no battery.

use std::os::raw::c_int;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ztnative::{Args, Ctx, Host, declare_plugin};

/// How long a reading is reused before the platform is asked again. The status
/// line redraws far more often than a battery moves.
const TTL: Duration = Duration::from_secs(30);

/// A battery reading: percent full, and what the battery is doing.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Reading {
    percent: u8,
    state: State,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Charging,
    Discharging,
    Charged,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            State::Charging => "charging",
            State::Discharging => "discharging",
            State::Charged => "charged",
        }
    }
}

/// The cached reading and when it was taken. `None` after a failed read, which
/// is also cached — a machine with no battery must not fork a process per
/// redraw just to be told so again.
static CACHE: Mutex<Option<(Instant, Option<Reading>)>> = Mutex::new(None);

/// The current reading, from the cache when it is fresh enough.
fn reading() -> Option<Reading> {
    let mut cache = CACHE.lock().ok()?;
    if let Some((taken, value)) = *cache {
        if taken.elapsed() < TTL {
            return value;
        }
    }
    let value = read_platform();
    *cache = Some((Instant::now(), value));
    value
}

#[cfg(target_os = "macos")]
fn read_platform() -> Option<Reading> {
    // `pmset -g batt` prints, for a laptop:
    //   Now drawing from 'Battery Power'
    //    -InternalBattery-0 (id=…)\t87%; discharging; 4:41 remaining present: true
    let out = std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.contains('%'))?;
    let percent = line
        .split('%')
        .next()?
        .rsplit(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let state = if line.contains("charged") {
        State::Charged
    } else if line.contains("discharging") {
        State::Discharging
    } else if line.contains("charging") || line.contains("AC attached") {
        State::Charging
    } else {
        State::Discharging
    };
    Some(Reading { percent, state })
}

#[cfg(target_os = "linux")]
fn read_platform() -> Option<Reading> {
    // sysfs, so this one costs no process at all: /sys/class/power_supply/BAT0
    // (or BAT1, or a vendor name) carries `capacity` and `status`.
    let supplies = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in supplies.flatten() {
        let dir = entry.path();
        let Ok(kind) = std::fs::read_to_string(dir.join("type")) else {
            continue;
        };
        if kind.trim() != "Battery" {
            continue;
        }
        let Ok(capacity) = std::fs::read_to_string(dir.join("capacity")) else {
            continue;
        };
        let Ok(percent) = capacity.trim().parse::<u8>() else {
            continue;
        };
        let status = std::fs::read_to_string(dir.join("status")).unwrap_or_default();
        let state = match status.trim() {
            "Charging" => State::Charging,
            "Full" => State::Charged,
            _ => State::Discharging,
        };
        return Some(Reading { percent, state });
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn read_platform() -> Option<Reading> {
    None
}

/// The prefix an option asks for, with the tmux-plugin convention of an
/// `@`-prefixed user option and a default when it is unset.
fn prefix(host: &Host, state: State) -> String {
    let (name, default) = match state {
        State::Charging => ("@battery-charging-prefix", "AC "),
        State::Discharging => ("@battery-discharging-prefix", "BAT "),
        State::Charged => ("@battery-charged-prefix", "FULL "),
    };
    host.get_option(name).unwrap_or_else(|| default.to_string())
}

/// `#{plugin_battery}` — prefix plus percentage.
fn battery(host: &Host, _key: &str) -> Option<String> {
    let r = reading()?;
    Some(format!("{}{}%", prefix(host, r.state), r.percent))
}

/// `#{plugin_battery_pct}` — the number alone, for `#{e|>:…}` arithmetic and
/// conditional formats.
fn battery_pct(_host: &Host, _key: &str) -> Option<String> {
    Some(reading()?.percent.to_string())
}

/// `#{plugin_battery_state}` — `charging` / `discharging` / `charged`, for
/// `#{==:#{plugin_battery_state},charging}` style conditionals.
fn battery_state(_host: &Host, _key: &str) -> Option<String> {
    Some(reading()?.state.as_str().to_string())
}

/// `battery [-r]` — print the reading; `-r` forces a fresh one, ignoring the
/// cache. Handy to check the plugin is alive without touching the status line.
fn battery_cmd(host: &Host, ctx: &Ctx, _args: &Args) -> c_int {
    if ctx.has('r') {
        if let Ok(mut cache) = CACHE.lock() {
            *cache = None;
        }
    }
    match reading() {
        Some(r) => {
            host.print(ctx, &format!("{}% {}", r.percent, r.state.as_str()));
            0
        }
        None => {
            host.error(ctx, "battery: no battery found");
            1
        }
    }
}

declare_plugin! {
    name: "battery",
    version: "0.1.0",
    commands: {
        "battery" => { template: "r", usage: "[-r]", handler: battery_cmd },
    },
    formats: {
        "plugin_battery" => battery,
        "plugin_battery_pct" => battery_pct,
        "plugin_battery_state" => battery_state,
    },
}

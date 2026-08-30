# AsusTuner

**English** | [简体中文](README.md)

A Linux control center for **ASUS TUF / ROG / Zephyrus** gaming laptops, inspired by [G-Helper](https://github.com/seerge/g-helper). Instead of reimplementing the hardware layer, AsusTuner drives what your system already has — the official `asusd` daemon and `ryzenadj` — so there is no bloat and no duplicate plumbing.

Tested on: ASUS TUF Gaming A16 FA607PV (Ryzen 9 7940HX + RTX 4060 Laptop) · Arch Linux · KDE Wayland.

## Design principles

1. **No hardware re-implementation.** `asusd` already manages fan curves, platform profiles, CPU EPP, charge limits and keyboard lighting; `ryzenadj` handles AMD power limits and Curve Optimizer; the `asus-armoury` firmware attributes expose power limits and GPU modes. AsusTuner is a **frontend** on top of them.
2. **Prefer existing interfaces** (asusd D-Bus, armoury sysfs, ryzenadj CLI, plain sysfs) over writing new ones.
3. **Every write is verified** — read back / behavior-validated, not just "it compiles".
4. **No model-specific hardcoding** — capabilities are detected at runtime through the asusd / armoury ecosystem.

The only self-built component is a thin **privileged backend**: it does not talk to hardware on its own — it just runs the same asusd / ryzenadj / armoury calls as root when needed, keeps state consistent, and re-applies your settings after boot or sleep.

## Components

| Component | What it is |
|---|---|
| `asustuner-backend` | Resident root service (systemd). JSON-line protocol over a Unix socket; persists state to `/var/lib/asustuner/state.json`; re-applies settings on boot and on wake-up (logind); 45 s drift-verify loop; singleton guard. |
| `asustuner-gui` | Qt6 / QML main window (cxx-qt). Five tabs: Performance, Fans, GPU, Battery, Monitor. All writes are routed through the backend. |
| `asustuner-tray` | Lightweight tray icon (ksni/SNI, no Qt). Right-click menu: profiles (● marks the active one), fan presets, restore config, open GUI, quit. |
| `asustuner-cli` | Command-line client; talks to asusd D-Bus and ryzenadj directly. |

## Features

| Feature | How | Status |
|---|---|---|
| Platform profiles (quiet / balanced / performance) | asusd `xyz.ljones.Platform` `PlatformProfile` | ✅ |
| Custom profiles, G-Helper style | Named profile = base platform + snapshot (power limits, undervolt, temp targets, fan curves, charge limit). The three built-ins cannot be deleted; saving under the same name overwrites their snapshot. Listed dynamically in GUI and tray. | ✅ |
| Power limits PL1/PL2/PL3 (STAPM/FAST/SLOW) | `asus-armoury` firmware attributes; ryzenadj fallback on models without armoury. Not persistent in firmware — reapplied by the backend on boot. | ✅ |
| CPU Curve Optimizer undervolt | `ryzenadj --set-coall` (iGPU `--set-cogfx` where the CPU supports it) | ✅ |
| Temperature targets (CPU / dGPU) | `ryzenadj --tctl-temp` / armoury `nv_temp_target` | ✅ |
| dGPU dynamic boost & MUX mode (Eco / Hybrid / dGPU-only) | armoury `nv_dynamic_boost` / `gpu_mux_mode` / `dgpu_disable`, with a pending-reboot notice | ✅ |
| Fan curves (dual-fan 8-point drag editor + full-speed calibration) | asusd `xyz.ljones.FanCurves` | ✅ |
| Charge limit & battery health | asusd `ChargeControlEndThreshold` / sysfs `power_supply` | ✅ |
| Keyboard RGB (Aura effect, speed, color, brightness) | root backend → sysfs `kbd_rgb_mode` / LED brightness | ✅ |
| Live monitoring (temps, CPU power via RAPL, fan RPM, battery) | read-only sysfs; the root-only RAPL counter is read by the backend | ✅ |
| Persistence across reboot / sleep | backend state replay + systemd service | ✅ |

## Building & installing

Arch / Arch-based:

```bash
sudo pacman -S cargo rust cmake qt6-base qt6-declarative asusd ryzenadj
git clone https://github.com/wu5810222-alt/AsusTuner.git
cd AsusTuner
sudo ./install.sh
```

`install.sh` builds the release binaries, installs all four to `/usr/bin`, enables `asustuner-backend.service`, and adds the tray to desktop autostart. Afterwards you get a tray icon in the panel; the main window opens from the tray.

Running just the GUI from a source tree (development):

```bash
cargo build --release
./run.sh
```

Environment overrides (development): `ASUSTUNER_SOCKET` / `ASUSTUNER_STATE` (custom socket/state paths), `ASUSTUNER_VERIFY_SECS` (drift-verify interval), `ASUSTUNER_NO_AUTH=1` (skip the polkit prompt when no backend service is deployed — the GUI then falls back to launching a root backend via pkexec for privileged operations).

## CLI

```bash
asustuner-cli sensors               # live monitoring
asustuner-cli profile               # current platform profile
asustuner-cli set-profile balanced  # quiet / balanced / performance / lowpower
asustuner-cli charge 80             # charge limit (%)
asustuner-cli power 45000 65000 45000  # power limits in mW: STAPM FAST SLOW (needs root)
asustuner-cli curve -30 -20         # Curve Optimizer undervolt, negative = undervolt (needs root)
asustuner-cli fan-get 0             # read the fan curve of the active profile
asustuner-cli fan-set --temp "56,61,66,71,76,80,85,97" --pwm "0,3,23,51,56,120,180,229"  # set CPU fan curve (add --gpu)
```

Profile / charge / fan commands go straight to asusd and work unprivileged; `power` and `curve` shell out to ryzenadj and need root.

## Permissions

- **asusd D-Bus** (profiles, charge limit, fan curves): readable *and writable* as a normal user — no root needed.
- **asus-armoury firmware attributes** (`/sys/class/firmware-attributes/asus-armoury/attributes/`): world-readable, root-writable → performed by the resident backend.
- **ryzenadj** (undervolt, legacy power path): needs root (`/dev/mem`) → performed by the backend.
- **Sensors** (k10temp, amdgpu, fan RPM, battery, cpufreq): plain read-only sysfs, no root.
- **RAPL CPU power** is root-only; the backend reads it for the monitor tab.

## Adapting to other models

AsusTuner works through the generic asusd / asus-armoury / ryzenadj interfaces, so any model supported by asusd (TUF, ROG, Zephyrus, …) should work without code changes. Capabilities are probed at runtime: firmware attributes your model lacks are hidden, and the power-limit sliders fall back to the ryzenadj path when armoury is unavailable.

## License

GPL-3.0-or-later

## Credits

- [asusctl / asusd](https://gitlab.com/asus-linux/asusctl) — the official ASUS Linux daemon
- [RyzenAdj](https://github.com/FlyGoat/RyzenAdj) — AMD power management tuning
- [G-Helper](https://github.com/seerge/g-helper) — feature inspiration

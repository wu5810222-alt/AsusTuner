# AsusTuner

**English** | [简体中文](README.zh-CN.md)

On the very top I want to declare that this project is totally made by AI, and I'm using glm-5.3-flash. I have already tested on my computer that every function is working well. Actually I haven't learned Rust, I just know that it's a light and quick language, so I think it may be suitable for a system widget.

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

## Dependencies

**Build (all distros):** Rust toolchain · a C++ compiler (gcc/clang — cxx-qt compiles generated C++) · Qt 6 ≥ 6.5 (`qt6-base` headers, `qt6-declarative` for QtQuick/QML, `qt6-wayland` on Wayland sessions). polkit/pkexec is preinstalled on every desktop distro.

**Runtime (probed at runtime, missing = graceful degradation):**
- `asusd` (package `asusctl`) — **required on ASUS models** for profiles / charge limit / fan curves / lighting / GPU modes.
- `ryzenadj` — **AMD CPUs only**, optional: Curve Optimizer undervolt, Tctl temp target, power-limit fallback.

> ⚠️ **Tested only on the author's machine** (Arch + KDE Wayland, FA607PV). The Debian / Fedora / openSUSE package names and the desktop-environment notes further down are compiled from upstream docs, not verified on real machines — if anything is off, PRs are very welcome.

```bash
# Arch / Arch-based
sudo pacman -S --needed base-devel rust qt6-base qt6-declarative qt6-wayland asusctl
# ryzenadj (AMD only) lives in the AUR:
paru -S ryzenadj                    # or yay -S ryzenadj

# Debian / Ubuntu (24.04+, Qt6 splits QML into modules)
sudo apt install build-essential cargo qt6-base-dev qt6-declarative-dev qt6-wayland \
  qml6-module-qtquick qml6-module-qtquick-controls qml6-module-qtquick-layouts \
  qml6-module-qtquick-templates qml6-module-qtquick-window qml6-module-qtqml-workerscript
# ^ package names not verified on real hardware (author only has an Arch machine) — PRs welcome
# asusctl is not packaged for Debian/Ubuntu — build from source: https://asus-linux.org
# ryzenadj (AMD only), no package — build:
sudo apt install cmake libpci-dev
git clone https://github.com/FlyGoat/RyzenAdj && cd RyzenAdj \
  && cmake -B build && cmake --build build && sudo cmake --install build

# Fedora
sudo dnf install rust cargo gcc-c++ qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtwayland
sudo dnf copr enable lukenukem/asus-linux && sudo dnf install asusctl   # ^ COPR name unverified
# ryzenadj (AMD only): sudo dnf install cmake libpci-devel, then build as above

# openSUSE
sudo zypper install rust cargo gcc-c++ qt6-base-devel qt6-declarative-devel qt6-wayland   # ^ unverified
# asusctl: OBS repo (see asus-linux.org); ryzenadj: build from source as above
```

NixOS: nixpkgs ships `asusctl` (`services.hardware.asusd`); give the build shell Qt6 (`qt6.qtbase` / `qt6.qtdeclarative` / `qt6.qtwayland`). Not packaged as a flake yet.

## Building & installing

```bash
git clone https://github.com/wu5810222-alt/AsusTuner.git
cd AsusTuner
sudo ./install.sh
```

`install.sh` builds the release binaries, installs all four to `/usr/bin`, enables `asustuner-backend.service`, installs the tray's desktop autostart **and** a systemd user unit (`asustuner-tray.service`, installed but not enabled — for compositors without XDG autostart), and prints non-blocking warnings when it detects GNOME-without-appindicator, a missing `asusd` or a missing `ryzenadj`. Afterwards you get a tray icon in the panel; the main window opens from the tray.

## Desktop environment notes

| DE | Tray (SNI) | polkit agent | Notes |
|---|---|---|---|
| KDE Plasma | ✅ native | ✅ built-in | works out of the box |
| GNOME | ⚠️ needs AppIndicator | ✅ built-in | install `gnome-shell-extension-appindicator` (or from extensions.gnome.org) and enable it |
| XFCE / Cinnamon / MATE | ✅ | ✅ built-in | fine on X11 |
| Waybar + niri / sway / hyprland | ✅ Waybar `tray` module | ⚠️ start your own | tray autostart: `systemctl --user enable --now asustuner-tray` |
| Compositor without a panel | ❌ no tray | — | use `asustuner-cli`; the backend keeps state regardless |

On minimal compositors remember to autostart a **polkit agent** (e.g. `polkit-gnome-authentication-agent-1`, `lxpolkit`, `polkit-kde-agent`) — without one the GUI cannot elevate the backend; the GUI log panel spells this out when it happens. Qt styling outside KDE (optional): `adwaita-qt` / `qt6ct`.

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

## Hardware & platform compatibility

Compact summary — the full matrix (function × CPU vendor × desktop) lives in [COMPATIBILITY.md](COMPATIBILITY.md):

| Capability class | ASUS + AMD (tested) | ASUS + Intel | non-ASUS |
|---|---|---|---|
| Profiles, charge limit, fan curves, lighting, MUX / dGPU | ✅ asusd + armoury | ✅ same (independent of CPU vendor) | ❌ no asusd — roadmap |
| Power limits PL1/PL2/PL3 | ✅ armoury, ryzenadj fallback | ✅ armoury (no ryzenadj fallback on Intel) | ❌ vendor-specific |
| Curve Optimizer undervolt / Tctl temp target | ✅ ryzenadj | ❌ auto-hidden (Intel MSRs are locked anyway) | ❌ |
| Boost, sensors, battery, RAPL CPU power | ✅ standard Linux | ✅ standard Linux (coretemp / i915 probed) | ✅ standard Linux |

Everything is probed at runtime (`caps.rs`): firmware attributes your model lacks are hidden in the GUI, temperature sources and the RAPL domain are auto-detected (k10temp/coretemp, amdgpu/i915, `intel-rapl:0`/`amd-rapl:*`), and AMD-only controls disappear on Intel machines without error noise. `asusd`-supported models (TUF / ROG / Zephyrus / 天选 …) need no code changes; non-ASUS support is an architectural seam we've deliberately left open (see COMPATIBILITY.md §4).

## License

GPL-3.0-or-later

## Credits

- [asusctl / asusd](https://gitlab.com/asus-linux/asusctl) — the official ASUS Linux daemon
- [RyzenAdj](https://github.com/FlyGoat/RyzenAdj) — AMD power management tuning
- [G-Helper](https://github.com/seerge/g-helper) — feature inspiration

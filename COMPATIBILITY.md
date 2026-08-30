# AsusTuner 兼容性矩阵

> 2026-08-30 · 能力探测见 `crates/asustuner-backend/src/caps.rs`（运行时探测，非硬编码机型）
> 结论：**ASUS 笔记本（AMD 或 Intel CPU）开箱即用**；非 ASUS 机器在路线图上（预留了平台抽象层，见 §4）。

## 1. 功能 × 硬件平台

| 功能 | ASUS + AMD（已实测） | ASUS + Intel（代码就绪，待实测） | 非 ASUS |
|---|---|---|---|
| 性能档位（静音/平衡/性能/低功耗） | ✅ asusd | ✅ asusd（与 CPU 平台无关） | ❌ 无 asusd |
| 充电限制 | ✅ asusd | ✅ asusd | ❌（未来可走标准 `charge_control_end_threshold` sysfs） |
| 功率墙 PL1/PL2/PL3 | ✅ asus-armoury 固件接口 | ✅ 同左（华硕固件接口，CPU 无关） | ❌ 各家固件接口不同 |
| 功率墙回退路径 | ✅ ryzenadj（armoury 缺失时） | —（ryzenadj 不适用，caps 自动不回退） | ❌ |
| CPU 降压 (Curve Optimiser) | ✅ ryzenadj coall/cogfx | ❌ 隐藏（Intel 新 U 的降压 MSR 普遍被厂商锁定，业界无解） | ❌ |
| 温度墙 Tctl | ✅ ryzenadj tctl-temp | ❌ 隐藏（无等价物） | ❌ |
| CPU Boost 开关 | ✅ 标准 cpufreq（通用） | ✅ 标准 cpufreq（通用） | ✅ 标准 cpufreq |
| 风扇曲线（8 点可拖拽） | ✅ asusd FanCurves | ✅ 同左 | ❌ 各家私有，无通用接口 |
| 风扇满转速校准 | ✅（临时切性能档） | ✅ 同左 | ❌ |
| 风扇 RPM 显示 | ✅ hwmon name=asus | ✅ 同左 | ❌（各家 hwmon 名不同） |
| CPU 温度 | ✅ k10temp | ✅ coretemp（探测顺序自动） | ⚠️ 按探测列表 |
| iGPU 温度 | ✅ amdgpu | ✅ i915/xe（xe 待加探测） | ⚠️ 按探测列表 |
| CPU 瞬时功率 (RAPL) | ✅ intel-rapl:0 | ✅（name=="cpu" 域探测，含 amd-rapl:*） | ✅ 标准 RAPL（大部分机器） |
| 电池健康/充电限制显示 | ✅ 标准 BAT0 sysfs | ✅ 通用 | ✅ 通用 |
| GPU 模式 MUX / dGPU 功耗 | ✅ armoury nv_* | ✅ 同左 | ❌ |
| 键盘亮度 / Aura 灯效 | ✅ asus::kbd_backlight | ✅ 同左 | ❌（亮度可未来走通用 kbd_backlight） |
| CPU 频率 / 型号 | ✅ 标准 sysfs | ✅ 标准 sysfs | ✅ 标准 sysfs |

**ASUS+Intel 与本表的差异点**（即 Tier1 改造面，均已实现）：
温度源探测（k10temp→coretemp / amdgpu→i915）、RAPL 域探测、AMD 专属功能（降压/温度墙/ryzenadj 回退）自动隐藏或跳过（GUI `amd_adj` qproperty + 后端 caps 门控）。

## 2. 功能 × 桌面环境

| 依赖 | KDE Plasma | GNOME | Waybar 类（niri/sway/hyprland 常配） | 裸合成器（niri/hyprland 无面板） |
|---|---|---|---|---|
| GUI（Qt6/QML） | ✅ | ✅（建议装 adwaita-qt 或 qt6ct 统一样式） | ✅ | ✅（需 `qt6-wayland` 包） |
| 托盘（SNI 协议） | ✅ 原生 | ⚠️ 需 gnome-shell-extension-appindicator | ✅ Waybar tray 模块 | ❌ 无面板即无托盘（功能可走 CLI） |
| polkit 授权（pkexec） | ✅ 自带 agent | ✅ 自带 agent | 视配置 | ⚠️ 常缺失，需自启一个（如 polkit-gnome-authentication-agent-1）——GUI 日志会明确提示 |
| 后端（systemd system 服务） | ✅ 与桌面无关 | ✅ | ✅ | ✅ |
| 托盘自启动 | ✅ XDG autostart | ✅ | 视会话 | ⚠️ 用 user unit：`systemctl --user enable --now asustuner-tray` |
| 睡眠唤醒自愈（logind） | ✅ 与桌面无关 | ✅ | ✅ | ✅ |

## 3. 部署要求清单

- **必须**：systemd、polkit、`qt6-wayland`（Wayland 会话）；ASUS 机器需 `asusctl`(asusd) ≥ 6.x
- **可选**：`ryzenadj`（仅 AMD：降压/温度墙/功率墙回退；`/usr/sbin/ryzenadj`）
- `install.sh` 会在装完后就 GNOME/asusd/ryzenadj 缺失给出提示（不阻断）

## 4. 非 ASUS 路线图（预留抽象层）

已预留的接缝（本版不启用）：
- `caps.rs` 是唯一的"能力真相源"——GUI 裁剪与后端门控都已收敛到 caps 判定
- asusd/armoury 调用点集中（backend `main.rs` 的 `asusd_*`/`write_power` 函数族），可替换为：
  - 档位 → 标准 `/sys/firmware/acpi/platform_profile`（内核接口，asusd 底层同源）
  - 充电 → 标准 `/sys/class/power_supply/BAT*/charge_control_end_threshold`
  - 功率墙 → 各家固件接口（联想 DPTC 等，需逐家适配）
- 风扇曲线/灯效在非 ASUS 机器上无通用内核接口，只能按厂商逐家支持

## 5. 已知风险（单用户定位）

- 后端 socket 默认 `0666`，且提供 `exec`/`ryzenadj` root 直通命令（GUI 终端面板依赖）——**多用户机器上任意本地用户可以 root 执行命令**。收紧方式：单元里加 `Environment=ASUSTUNER_SOCK_MODE=660` + 共享组；或改协议去掉直通命令
- `coretemp`/`i915` 的 hwmon 命名在不同内核版本可能有别名（如 `zenpower`），探测列表按需扩充

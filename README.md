# AsusTuner

[English](README_EN.md) | **简体中文**

专为 **ASUS 天选 / TUF / ROG / 幻** 系列笔记本开发的 Linux 系统控制工具。
借鉴 [G-Helper](https://github.com/seerge/g-helper) 的功能，**直接调用系统已有的 `asusd`（华硕官方守护进程）和 `ryzenadj`**，摒弃 Armoury Crate 的庞杂，也不重复造轮子。

已在 ASUS TUF Gaming A16 FA607PV（7940HX + RTX 4060）· Arch Linux · KDE Wayland 上实测。

## 核心理念

**不自写硬件层，不重复实现。** 华硕 Linux 生态里 `asusd` 已经在管理风扇曲线、性能档位、CPU EPP、充电限制、键盘灯等硬件；`ryzenadj` 负责 AMD 功率墙和降压；asus-armoury 固件属性暴露功率墙与 GPU 模式。本工具只是**前端**——把这些现成接口接到用户手里。唯一的自建组件是一个**薄特权后端**：它不直接碰硬件，只在需要 root 时代为调用同样的系统接口（asusd / ryzenadj / 固件属性），并负责状态持久化与开机/唤醒重放。

## 功能一览

| 功能 | 实现方式 | 状态 |
|---|---|---|
| 性能档位（quiet/balanced/performance） | 解 asusd `xyz.ljones.Platform` 的 `PlatformProfile` | ✅ |
| 自定义配置方案（G-Helper 式） | 命名方案 = 基座档位 + 设置快照（功率墙/降压/温度墙/风扇曲线/充电限制）；内置三方案不可删、同名保存=覆盖；GUI/托盘动态列出 | ✅ |
| 功率墙 PL1/PL2/PL3（STAPM/FAST/SLOW） | asus-armoury 固件属性（无 armoury 机型回退 ryzenadj）；固件不跨重启，由后端开机重放 | ✅ |
| CPU 降压（Curve Optimiser） | `ryzenadj --set-coall`（iGPU `--set-cogfx` 视 CPU 支持） | ✅ |
| 温度墙（CPU/dGPU） | `ryzenadj --tctl-temp` / armoury `nv_temp_target` | ✅ |
| dGPU 动态加速 + GPU 模式（Eco/混合/独显直连） | armoury `nv_dynamic_boost`/`gpu_mux_mode`/`dgpu_disable`，含 pending_reboot 提示 | ✅ |
| 风扇曲线（双风扇 8 点拖拽编辑 + 满转速校准） | asusd `xyz.ljones.FanCurves` | ✅ |
| 电池充电限制 + 电池健康 | asusd `ChargeControlEndThreshold` / sysfs `power_supply` | ✅ |
| 键盘 RGB（Aura 效果/速度/颜色/亮度） | root 后端写 sysfs `kbd_rgb_mode` / LED 亮度 | ✅ |
| 实时监控（温度/频率/CPU 功率 RAPL/风扇转速/电池） | 只读 sysfs；仅 root 可读的 RAPL 由后端读取 | ✅ |
| 状态持久化（重启/唤醒自动恢复） | 后端 state.json 重放 + systemd 常驻服务 | ✅ |

## 架构（三件套 + CLI）

```
AsusTuner/
├── crates/
│   ├── asustuner-backend/   # root 常驻后端（systemd）：Unix socket JSON 行协议、
│   │                        #   state.json 持久化、开机/logind 唤醒重放、45s 漂移校验、单例守护
│   ├── asustuner-gui/       # QML + cxx-qt (Qt6) 主界面：性能/风扇/GPU/电池/监控 五页签，写操作全走后端
│   ├── asustuner-tray/      # ksni 托盘（无 Qt）：右键方案（●=生效中）/风扇预设/恢复配置/开 GUI/退出
│   └── asustuner-cli/       # 命令行客户端（直连 asusd + ryzenadj）
├── systemd/asustuner-backend.service
├── install.sh               # sudo 一次部署：编译 + 装二进制 + 起服务 + 托盘自启
├── run.sh                   # 开发用：编译 + 启动 GUI
└── scripts/                 # 实验脚本
```

### 调用的现有接口

- **asusd**：system bus 服务 `xyz.ljones.Asusd` @ `/xyz/ljones`
  - `Platform`：`PlatformProfile`、`ChargeControlEndThreshold`、`Profile*Epp`
  - `FanCurves`：`set_fan_curve`、`fan_curve_data`、`set_curves_to_defaults`
  - `Aura` @ `/xyz/ljones/Aura`：键盘 RGB（预留）
- **asus-armoury 固件属性**：`/sys/class/firmware-attributes/asus-armoury/attributes/`（ppt_pl1_spl / ppt_pl2_sppt / ppt_pl3_fppt / nv_temp_target / nv_dynamic_boost / gpu_mux_mode / dgpu_disable 等）
- **AMD 功率/降压**：`/usr/sbin/ryzenadj`（`--stapm-limit=`、`--set-coall=` 等，仅长选项带等号）
- **只读监控（免 root）**：k10temp / amdgpu hwmon 温度、hwmon name=asus 风扇转速、cpufreq 频率、`power_supply` 电池；CPU 瞬时功率走 RAPL（root，经后端）

## 系统依赖

```bash
sudo pacman -S cargo rust cmake qt6-base qt6-declarative asusd ryzenadj
```

## 安装（推荐）

```bash
git clone https://github.com/wu5810222-alt/AsusTuner.git
cd AsusTuner
sudo ./install.sh
```

`install.sh` 会编译 release、把四个二进制装到 `/usr/bin`、启用 `asustuner-backend.service`、并把托盘加入桌面自启动。之后托盘常驻面板，主窗口从托盘按需打开。

## 从源码运行 GUI（开发）

```bash
cargo build --release
./run.sh
```

开发环境变量：`ASUSTUNER_SOCKET` / `ASUSTUNER_STATE`（覆盖 socket/state 路径）、`ASUSTUNER_VERIFY_SECS`（校验轮询间隔）、`ASUSTUNER_NO_AUTH=1`（未部署后端服务时跳过 polkit 弹窗，GUI 会以 pkexec 兜底拉起 root 后端执行特权操作）。

**后端日志/终端面板**：GUI 右上角"后端日志/终端"可展开——实时显示后端进程输出（`◀` 应答 / `•` 日志 / `▶` 命令），底部输入行可以 root 执行任意命令（如 `ryzenadj -i`）。

**风扇曲线**：风扇页是可拖拽的双曲线编辑器（CPU/GPU 同图、8 点、温度轴 40-100°C、PWM 0-255、自动保持单调），支持预设与满转速校准，一键应用经 asusd 生效。

### CLI 用法

```bash
asustuner-cli sensors               # 实时监控
asustuner-cli profile               # 当前性能档位
asustuner-cli set-profile balanced  # 设置档位（quiet/balanced/performance/lowpower）
asustuner-cli charge 80             # 充电限制（%）
asustuner-cli power 45000 65000 45000  # 功率墙（mW：STAPM FAST SLOW，需 root）
asustuner-cli curve -30 -20         # CPU 降压（负值=降压，需 root）
asustuner-cli fan-get 0             # 读当前档位风扇曲线
asustuner-cli fan-set --temp "56,61,66,71,76,80,85,97" --pwm "0,3,23,51,56,120,180,229"  # 设置 CPU 风扇曲线（可加 --gpu）
```

档位/充电/风扇命令直连 asusd，普通用户可用；`power`/`curve` 走 ryzenadj，需要 root。

## 权限说明

- **asusd D-Bus**（档位/充电/风扇曲线）：普通用户**可读可写**（实测），无需 root。
- **asus-armoury 固件属性**：0644 世界可读、root 写 → 由常驻后端执行。
- **ryzenadj**（降压/功率）：需 root（`/dev/mem`）→ 由后端执行。
- **传感器监控**（k10temp/amdgpu/风扇/电池/cpufreq）：只读 sysfs，免 root。
- **RAPL CPU 功率**：仅 root 可读，由后端代读供监控页显示。

## 适配其它机型

本工具通过 asusd / asus-armoury / ryzenadj 的通用接口工作，天然适配所有被 asusd 支持的天选 / TUF / ROG / 幻机型，新增机型无需改核心代码。能力在运行时探测：机型缺失的固件属性自动隐藏，无 armoury 时功率墙滑条回退 ryzenadj 路径。

## 许可

GPL-3.0-or-later

## 致谢

- [asusctl / asusd](https://gitlab.com/asus-linux/asusctl)：华硕 Linux 官方守护进程
- [ryzenadj](https://github.com/FlyGoat/RyzenAdj)：AMD 电源管理调优
- [G-Helper](https://github.com/seerge/g-helper)：功能参考

# AsusTuner

专为 **ASUS 天选 / TUF / ROG / 幻** 系列笔记本开发的 Linux 系统控制工具。
借鉴 [G-Helper](https://github.com/seerge/g-helper) 的功能，**直接调用系统已有的 `asusd`（华硕官方守护进程）和 `ryzenadj`**，摒弃 Armoury Crate 的庞杂，也不重复造轮子。

## 核心理念

**不自建守护进程，不重新实现硬件层。** 华硕 Linux 生态里 `asusd` 已经在管理风扇曲线、性能档位、CPU EPP、充电限制、键盘灯等硬件；`ryzenadj` 负责 AMD 的功率墙和降压。本工具只是**前端**——把用户设置通过 `asusd` / `ryzenadj` 的 D-Bus / CLI 交互回硬件，并展示实时监控。

## 功能一览

| 功能 | 实现方式 | 状态 |
|---|---|---|
| 性能档位（quiet/balanced/performance） | 解 asusd `xyz.ljones.Platform` 的 `PlatformProfile` | ✅ |
| CPU 降压（Curve Optimiser） | `ryzenadj --set-coall/--set-cogfx`（负值=降压） | ✅ |
| 功率墙（STAPM/PPT） | `ryzenadj --stapm-limit/--fast-limit/--slow-limit` | ✅ |
| 电池充电限制 | asusd `ChargeControlEndThreshold` | ✅ |
| CPU EPP / 各档功耗设置 | asusd `Platform`（各 profile 的 EPP） | ✅ |
| 风扇曲线（双风扇 8 点编辑） | asusd `xyz.ljones.FanCurves`（`set_fan_curve`/`fan_curve_data`） | ✅ |
| 实时监控（温度/频率/电池） | 只读 sysfs（k10temp/amdgpu/BAT0） | ✅ |
| 键盘灯 RGB | asusd `xyz.ljones.Aura` | 🔌 界面待接 |

## 架构（精简）

前端（GUI/CLI）**直连系统已有的 asusd + ryzenadj**，无需自研守护进程、无需 root（asusd 本身以 root 运行）。

```
AsusTuner/
├── crates/
│   ├── asustuner-gui/     # QML + cxx-qt KDE 前端（直连 asusd + ryzenadj）
│   └── asustuner-cli/     # 命令行客户端（直连 asusd + ryzenadj）
├── run.sh                 # 一键启动 GUI
└── README.md
```

### 调用的现有接口

- **服务**：`xyz.ljones.Asusd`（system bus）
- **性能档位/充电/EPP**：`xyz.ljones.Platform` @ `/xyz/ljones`
  - `PlatformProfile`、`PlatformProfileChoices`、`ChargeControlEndThreshold`、`Profile*Epp`
- **风扇曲线**：`xyz.ljones.FanCurves` @ `/xyz/ljones`（`set_fan_curve`、`fan_curve_data`）
- **键盘灯**：`xyz.ljones.Aura` @ `/xyz/ljones/Aura`
- **AMD 功率/降压**：`/usr/sbin/ryzenadj`（`--stapm-limit`、`--set-coall` 等）

## 系统依赖

```bash
sudo pacman -S cargo rust cmake qt6-base qt6-declarative asusd ryzenadj
```

## 运行

```bash
cd AsusTuner
cargo build --release
./run.sh            # 启动 GUI
```

**启动授权**：GUI 启动时会弹出系统 polkit 密码框（pkexec），授权后拉起 root 后端 `asustuner-backend`——功率墙/降压/温度墙/CPU Boost/键盘灯需要它。取消授权也能用（性能档位/风扇曲线/充电限制/监控不受影响，右上角状态灯为橙色，点击可重新授权）。开发时 `ASUSTUNER_NO_AUTH=1` 跳过弹窗。

**后端日志/终端面板**：点右上角"后端日志/终端"展开——实时显示后端进程输出（`◀` 应答 / `•` 日志 / `▶` 命令），底部输入行可以 root 执行任意命令（如 `ryzenadj -i`）。

**风扇曲线**：风扇页是可拖拽的 8 点曲线编辑器（拖圆点、温度轴 40-100°C、PWM 0-255、自动保持单调），支持静音/均衡/激进预设与 CPU/GPU 切换，一键应用经 asusd 生效。

### CLI 用法

```bash
asustuner-cli sensors               # 实时监控
asustuner-cli profile               # 当前性能档位
asustuner-cli set-profile balanced  # 设置档位（quiet/balanced/performance/lowpower）
asustuner-cli charge 80             # 充电限制
asustuner-cli power 45000 65000 45000  # 功率墙（mW，需 ryzenadj 可访问硬件）
asustuner-cli curve -30 -20         # CPU 降压（负值=降压）
asustuner-cli fan-get 0             # 读当前档位风扇曲线
asustuner-cli fan-set --temp "56,61,66,71,76,80,85,97" --pwm "0,3,23,51,56,120,180,229"  # 设置 CPU 风扇曲线（可加 --gpu）
```

## 关于 ryzenadj 权限

`ryzenadj` 需要通过 `/dev/mem` 访问 SMU（功率墙/降压），普通用户无此权限。因此 `asustuner-cli power` / `curve` 需要 `sudo` 执行，或确保运行用户能访问 `/dev/mem`（通常加入 `kmem` 组或 root）。性能档位/充电限制/监控（走 asusd / sysfs）则无需 root。

## 适配其它机型

本工具通过 `asusd` / `ryzenadj` 的通用接口工作，天然适配所有被 asusd 支持的天选 / TUF / ROG / 幻机型。新增机型无需改核心代码。

## 许可

GPL-3.0-or-later

## 致谢

- [asusctl / asusd](https://gitlab.com/asus-linux/asusctl)：华硕 Linux 官方守护进程
- [ryzenadj](https://github.com/FlyGoat/RyzenAdj)：AMD 电源管理调优
- [G-Helper](https://github.com/seerge/g-helper)：功能参考

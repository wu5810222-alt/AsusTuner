# AsusTuner Check-in（交接与改进基线）

> 更新日期：2026-08-29 · 平台：ASUS TUF Gaming A16 FA607PV · KDE Wayland · Arch Linux
> 用途：新会话/新维护者快速接手；改进前先读 §6 教训与 §9 计划。

---

## 1. 项目定位与设计原则

**一句话**：华硕天选/TUF/ROG 笔记本的 Linux 控制前端，功能对标 G-Helper。

**核心设计原则（不可违背）**：
1. **不自建守护进程、不自写 sysfs 硬件层**——asusd（系统已有，root 运行）管风扇曲线/性能档位/EPP/充电限制/键盘灯；ryzenadj 管 AMD 功率墙/降压。本项目只是**前端**。
2. **能调用就不重复实现**——优先调 asusd 的 D-Bus、ryzenadj 的 CLI。
3. **每个写操作必须验证生效**（读回确认），不能只验证"能编译/能弹窗"。
4. 接口不写死机型，通过运行时探测/读 asusd 适配。

## 2. 当前架构

```
AsusTuner/
├── crates/
│   ├── asustuner-gui/    # QML + cxx-qt (Qt6) 前端，直连 asusd D-Bus + 调 ryzenadj
│   │   ├── src/main.rs           # Qt 入口
│   │   ├── src/cxxqt_object.rs   # QObject 桥（属性/invokable）
│   │   ├── src/fan_curves.rs     # asusd FanCurves 接口类型复刻 + zbus proxy
│   │   └── qml/main.qml          # 界面
│   ├── asustuner-cli/    # 命令行（直连 asusd + ryzenadj）
│   │   ├── src/main.rs
│   │   └── src/fan.rs            # FanCurves 类型 + 读写封装
│   └── asustuner-backend/ # root 特权后端（JSON 行协议 over stdio，pkexec 启动）
├── run.sh                # 一键构建+启动 GUI
├── CHECKIN.md            # 本文件
└── README.md
```

**数据流**：GUI/CLI → (D-Bus) asusd → 硬件；GUI/CLI → (subprocess) ryzenadj → SMU；GUI → (只读 sysfs) 传感器监控。

**已删除**（历史上走过弯路，勿恢复）：asustuner-daemon（自研守护进程）、asustuner-shared（协议库）、自写 hardware/* sysfs 层、systemd/ 自建服务、config/ 档位文件。

## 3. 硬件基线（本机实测）

| 项 | 值 |
|---|---|
| 机型 | ASUS TUF Gaming A16 FA607PV（DMI board_name=FA607PV） |
| CPU | Ryzen 9 7940HX，32 线程，amd-pstate-epp 驱动 |
| dGPU | RTX 4060 Laptop（nvml 可用，暂未接） |
| 温度源 | k10temp（CPU）、amdgpu hwmon（iGPU）、/dev/mem 走 ryzenadj |
| 风扇 | hwmon name=asus：fan1_input/fan2_input（RPM，只读免 root） |
| 键盘灯 | /sys/class/leds/asus::kbd_backlight/brightness（0-3，root 可写） |
| CPU boost | /sys/devices/system/cpu/cpufreq/boost（root） |
| 系统 | Rust 1.98 · Qt 6.11.2 · zbus 4.4 · cxx-qt 0.10 · asusd 6.3.8 · ryzenadj 0.19.0 |

## 4. 功能状态表

| 功能 | 实现路径 | 状态 |
|---|---|---|
| 性能档位 quiet/balanced/performance | asusd `PlatformProfile` (property, u32) | ✅ CLI 读回验证 + gdbus 验证 |
| 充电限制 60/80/100 | asusd `ChargeControlEndThreshold` (property, u8) | ✅ 接口通（GUI 按钮） |
| 风扇曲线读 | asusd `fan_curve_data(profile)` | ✅ CLI fan-get 验证 |
| 风扇曲线写/恢复默认 | asusd `set_fan_curve` / `set_curves_to_defaults` | ✅ CLI fan-set 写回+读回验证 |
| GUI 全部界面 | QML | ✅ 截图视觉验证（监控/档位/降压/功率墙/风扇曲线/充电） |
| 实时监控 | 只读 sysfs | ✅ |
| 功率墙 STAPM/FAST/SLOW | GUI→root 后端→ryzenadj `--stapm-limit=…` | ✅ 后端协议验证；root 授权后生效 |
| CPU 降压 Curve Optimiser | GUI→root 后端→ryzenadj `--set-coall= --set-cogfx=` | ✅ 同上 |
| 温度墙 Tctl / CPU Boost / 键盘灯 | root 后端（sysfs/ryzenadj） | ✅ 已接（UI: 性能页） |
| 风扇曲线图形编辑器 | QML Canvas 8 点可拖拽 + 预设 + CPU/GPU 切换 | ✅ 已实现（写链路经 CLI 验证） |
| 后端日志/终端面板 | GUI 底部可折叠面板：后端 stdout/stderr + root 命令行 | ✅ 已实现 |
| GPU 模式（supergfxctl） | supergfxd 本机未启用 | ❌ 留接口 |

## 5. 关键接口速查

### asusd（system bus，服务名 `xyz.ljones.Asusd`，普通用户可读写——已实测）
- 路径 `/xyz/ljones`，多接口同路径按 interface 名区分：
- **`xyz.ljones.Platform`**（property 经 `org.freedesktop.DBus.Properties.Get/Set`）：
  - `PlatformProfile` u32：0=Balanced 1=Performance 2=Quiet 3=LowPower
  - `PlatformProfileChoices` → [u32]
  - `ChargeControlEndThreshold` u8（充电限制 %）
  - `ProfileQuietEpp/BalancedEpp/PerformanceEpp` u32
- **`xyz.ljones.FanCurves`**（方法调用）：
  - `fan_curve_data(profile:u32) -> Vec<CurveData>`
  - `set_fan_curve(profile:u32, curve:CurveData)`（自动激活）
  - `set_curves_to_defaults(profile:u32)`
  - `CurveData = { fan: FanCurvePU, pwm: [u8;8], temp: [u8;8], enabled: bool }`
  - **`FanCurvePU` 在 D-Bus 上是字符串签名 "s"**（"CPU"/"GPU"/"MID"），不是整数！`PlatformProfile` 是 u32 签名 "u"。
- `xyz.ljones.Aura` @ `/xyz/ljones/Aura`：键盘 RGB（未接）
- 本地复刻见 `crates/*/src/fan_curves.rs`（zbus4 `#[proxy]` + `#[derive(Type)]`，已验证可用）

### ryzenadj 0.19.0
- **只能用长选项带等号**：`--stapm-limit=45000`；短选项粘等号 `-a=45000` 会报 "expects an unsigned 32-bit integer"（源码 argparse 证实）
- 降压：`--set-coall=-30`（负值合法，strtol 解析为 int）；iGPU：`--set-cogfx=`
- 温度墙：`--tctl-temp=90`
- **需要 root**（/dev/mem，root:kmem）；普通用户报 "no compatible ryzen_smu kernel module found, fallback to /dev/mem" + pcilib 权限错
- 二进制：/usr/sbin/ryzenadj

### 只读监控（免 root）
- 温度：/sys/class/hwmon/*/name 匹配 k10temp、amdgpu → temp1_input（毫度）
- 风扇 RPM：hwmon name=asus → fan1_input/fan2_input
- CPU 频率：/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq（kHz）
- 电池：/sys/class/power_supply/BAT0/{capacity,status}

## 6. 教训（血泪，必读）

1. **过度设计是大坑**：最初建了"特权守护进程+协议库+自写 sysfs 层"三层架构（2400 行），而 asusd 现成接口全都有。已全部删除，精简到 ~700 行。**新增功能先查 asusd 有没有现成接口。**
2. **写操作必须读回验证**：曾只验证"读监控+弹窗"，实际所有写操作因 root 权限静默失败，用户反馈"完全起不到作用"。
3. **普通用户可直连 asusd 写**（gdbus 实测 PlatformProfile Set 生效），不要想当然认为 D-Bus 系统服务都要 root。
4. **ryzenadj 参数格式**：短选项粘等号不被解析（用长选项）。
5. **cxx-qt bridge 语法限制**：bridge 宏展开的文件里**不能用 let-else**（报 expected `;`，用 match）；QString→String 用 `.into()`（无 .as_str()）；qproperty 名用 snake_case（getter/getXxx、C++ setter 是 PascalCase）；改属性的 invokable 签名 `self: Pin<&mut Self>` 且实现里用 `self.as_mut().set_xxx()`；QML `console.log` 不进 stderr，验证 QML JS 执行需在 Rust invokable 里 eprintln。
6. **zvariant derive**：zvariant 4 没有 `derive` feature（宏默认可用），Cargo.toml 里写 `features=["derive"]` 会解析失败。
7. **后台调试**：Wayland 下 wmctrl/xdotool 看不到窗口；截图用 `spectacle -b -n -f` 全屏 + convert 裁剪；后台进程用 run_in_background 或 setsid，别混管道。
8. **验证 UI 用视觉**：启动 GUI 后 spectacle 截图读图确认，比只看日志可靠。

## 7. 构建 / 运行 / 调试

```bash
cd /home/guts/Projects/AsusTuner
cargo build --release          # 全量构建（GUI ~1.5min）
./run.sh                       # 构建+启动 GUI
cargo test                     # 单测（gui fan_curves 2 个）
# CLI 直测（asusd 功能免 root）：
target/release/asustuner-cli profile                 # 读档位
target/release/asustuner-cli set-profile performance # 写档位（读回验证）
target/release/asustuner-cli sensors                 # 监控
target/release/asustuner-cli fan-get 0               # 读风扇曲线
target/release/asustuner-cli fan-set --temp "56,61,66,71,76,80,85,97" --pwm "0,3,23,51,56,120,180,229"
# gdbus 直调 asusd（调试利器）：
gdbus call --system --dest xyz.ljones.Asusd --object-path /xyz/ljones \
  --method org.freedesktop.DBus.Properties.Get xyz.ljones.Platform PlatformProfile
# GUI 调试：RUST_LOG 无效（Qt app），用 spectacle 截图 + eprintln
```

## 8. 已知问题与限制

- **ryzenadj 类操作（功率墙/降压/boost/键盘灯）需 root**——这是当前最大功能缺口 → §9 方案解决。
- GUI 风扇曲线 UI 目前 CPU/GPU 共用一套预设；GPU 单独曲线字段已留（setFanCurve 支持空 GPU 参数只设 CPU）。
- 风扇曲线"当前曲线摘要"在窗口过矮时被裁剪（窗口高 840 已缓解）。
- supergfxd 本机未启用，GPU 模式切换未接。
- nvml（dGPU 温度/功耗）未接。

## 9. 下一步计划

**已完成（2026-08-29 本轮）**：
- ✅ asustuner-backend（JSON stdio：ping/ryzenadj/exec/boost/kbd/exit；协议自测通过）
- ✅ GUI 启动自动 pkexec 拉起后端（polkit 原生密码框；`ASUSTUNER_NO_AUTH=1` 跳过供开发）
- ✅ G-Helper 风格 UI：头部（状态灯=点击授权 + 日志面板开关）+ 模式栏（当前档位高亮）+ 四页签 + 底部实时状态条
- ✅ 可折叠后端日志/终端面板（stdout◀/stderr•/命令▶ + root 命令输入行）
- ✅ 风扇页改为 Canvas 可拖拽曲线编辑器（8 点、40-100°C×0-255、邻居钳位单调、预设、CPU/GPU 切换）

**2026-08-29 三轮（用户反馈修复）**：
- ✅ 风扇编辑器改**双曲线同图**（CPU 蓝/GPU 绿，16 点就近抓取拖拽，图例；CurveEditor 持 cpuPoints+gpuPoints 双数组）
- ✅ 日志面板开关时窗口高度自适应 ±210（修复默认尺寸下面板显示不全）；默认窗口 780x680
- ✅ 监控页 + 底部状态条加 **CPU 瞬时功率**：后端新增 `rapl` 命令读 `/sys/class/powercap/intel-rapl:0/energy_uj`（root-only），GUI 差分算瓦数（回绕用 max_energy_range_uj 修正，compute_cpu_power 有单测）；未授权显示"—/需授权后端"

**2026-08-30 四轮（用户反馈修复）**：
- ✅ 布局根治：四页全部 Flickable 包裹 + StackLayout `Layout.minimumHeight: 0`——内容在页内滚动，状态条/日志面板不再被挤出窗口（默认 680 高即可见状态条）
- ✅ 键盘灯效 (Aura)：性能页新组——效果(静态/呼吸/闪烁/彩虹)+速度(慢/中/快)+9 色色块(选中描边)+亮度+应用。走 root 后端 `aura` 命令写 `kbd_rgb_mode` 六字节 `[1,mode,r,g,b,speed]`（字节格式与 asusd `aura_laptop/mod.rs write_effect_and_apply` 一致；Speed 枚举 0xe1/0xeb/0xf5 来自 rog-aura builtin_modes.rs；sysfs 写入格式为空格分隔十进制）。亮度走既有 `kbd` 命令。彩虹模式通常忽略颜色。v2 可选：切 asusd `xyz.ljones.Aura` D-Bus（需复刻 AuraEffect 嵌套类型）
- ✅ 风扇预设 CPU/GPU 分开（两套表，GPU 低温区略缓）
- ✅ CPU 功率实测生效（用户截图状态条显示 25W/24W，RAPL 链路工作）

**2026-08-30 五轮（方案 B 三件套落地）**：
- ✅ 后端常驻化：Unix socket（多客户端）+ 状态持久化(/var/lib/asustuner/state.json) + **启动自动恢复**（实测：启动日志"档位 quiet: true"）+ logind 唤醒重放 + asusd 档位锁定自愈（可 set_lock_profile 开关，默认开）
- ✅ 轻托盘 asustuner-tray：ksni(SNI, blocking+async-io, 无Qt ~3MB)；右键=档位RadioGroup/风扇预设子菜单(独立CPU·GPU表)/恢复配置/打开主界面/退出；22x22 代码绘图标；**已在 KDE 面板实测显示**（D-Bus 注册 `org.kde.StatusNotifierItem-<pid>` 验证 + 截图）
- ✅ GUI 改造：子管道→socket 客户端；全部写操作路由经后端（状态一致性）；未部署服务时 pkexec 兜底；托盘 GUI 路径解析 current_exe 兄弟目录
- ✅ install.sh：sudo 一次部署（4 二进制 + systemd 服务 enable --now + 托盘 /etc/xdg/autostart）
- 开发调试：ASUSTUNER_SOCKET / ASUSTUNER_STATE 环境变量可覆盖路径

**2026-08-30 七轮（上游调研裁决 + armoury 落地，Q1-Q6 已裁决）**：
- 调研输入：RESEARCH.md（上游项目调研，含关键自我修正——"事件驱动持久化更优雅"被推翻：ppd 直写 sysfs 可能绕过 asusd 事件链路）
- ✅ 混合持久化落地（Q3→方案③）：backend `verify_loop()` 每 45s（ASUSTUNER_VERIFY_SECS 可覆盖）比对档位漂移才写回，补齐事件驱动不可靠的兜底腿；**单例守护**（main 入口 socket 探测，双实例会互相夺回打架——实测发现并修复）；RGB 重放 150ms 间隔（调研 C 项 z-helper 经验）
- ✅ 功率墙接 armoury 读回（Q1 读半场）：sysfs 0644 世界可读免 root，GUI 三滑条 PL1/PL2/PL3 瓦特+固件范围(30-135W 步进1)，实测初始值对齐 100/115/135W；不可用机型回退旧 mW SpinBox；**写路径仍 ryzenadj，迁移等写入实验**
- ✅ 实验脚本 scripts/armoury-experiment.sh（sudo 跑）：T1 同值通道/T2 +5W+ryzenadj 交叉对读/T3 档位联动（EnablePptGroup 暗示 ppt 可能挂档位下）；尾部含 D-Bus 免根写入试探（gdbus Set AsusArmoury CurrentValue）与重启持久化检查指引
- 裁决记录：Q2 dGPU 纳入路线图(依赖实验)/Q4 托盘已落地不再裁决/Q5 电池健康页排期/Q6 定位=「G-Helper 体验+asusd 缺口」不做 rog-control-center 替代品，GPU 模式优先 armoury gpu_mux_mode/dgpu_disable（接口已实测存在）而非 supergfxd

**2026-08-30 八轮（实验结果落地：功率墙迁移 armoury，commit 1e941d0）**：
用户实验实测：T1 写入通道✓ / T2 固件接受并保持(+5W)✓ / T3 与档位解耦（切档不重写）✓ / **D-Bus 免根写入可用**（asusd AsusArmoury CurrentValue Set 普通用户无报错）/ **固件不跨重启持久**（回 100）。
落地：backend `write_power()` = armoury sysfs 三写（stapm→PL1/fast→PL3/slow→PL2，mW→W，150ms 固件间隔），`set_power`/`apply_state` 均改走它；无 armoury 机型回退 ryzenadj；**ryzenadj 保留**降压/温度墙/终端直通。功率墙**保留在启动重放名单**（不持久已证实）。行为级验证方法：功率墙设 45W → 跑压力 → 监控页 CPU 功率应压在 ~45W（RAPL 实测）。
已知备选路径：asusd D-Bus 免根写 armoury（GUI/CLI 将来可不依赖后端写功率墙；CLI 免 sudo 路径候选）。

**2026-08-30 九轮（行为级验证✓ + 各档位功耗现象记录 + 下一批开工）**：
行为级验证通过：功率墙写入链路（GUI→后端→armoury 固件）真实限住 CPU。
**现象记录（UX 优化项，暂不修）**：安静模式 CPU 几乎不超 30W / 平衡精确锁 45W / 性能模式锁 85W。机制假设（只读探查证实一半）：asusd 三档 EPP 不同（Quiet=Power(4)/Balanced=BalancePower(3)/Performance=Performance(1)），PPT 是天花板非目标——静音档 EPP 压得不冲上限、性能档全力冲撞 FPPT(点 45W 预设时 PL3=65)+STAPM 平均窗时序（85W 峰值在平均窗收敛前），ppd 切换还会独立重设 EPP 放大差异。待办：性能档专项实验（T3 当时只测了 quiet↔balanced）+ 文档向用户说明"调功耗请用平衡档"。

**2026-08-30 十二轮（降压 cogfx 修复，9800575）**：用户实测发现降压 ok:false 但 coall 实际生效——Dragon Range（7940HX）不支持 cogfx(iGPU 降压)，GUI 同时发 --set-coall/--set-cogfx，cogfx 失败拉低整命令退出码。修复：backend set_curve/apply_state 把 coall/cogfx 拆成独立 ryzenadj 调用（各报成败，cogfx 仅自身成功才入状态）；GUI 滑块只发 all_cores（后端 igpu 能力保留给其他 family）。另注：ryzenadj 打印 4294967291 = -5 的 u32 补码，SMU 按有符号解释，数值正确非 bug。

**待办（优先级序）**：
1. **用户执行**：`sudo chown -R guts:guts ~/Projects/AsusTuner/target && sudo ./install.sh`（重装新功能 + 构建修复生效）
2. 性能档功耗行为专项实验（85W 现象：FPPT/STAPM 窗/ppd EPP 交互）
3. dGPU 写入行为压测（温度墙/动态加速生效验证）
4. GUI 拆紧凑模式 / CLI power 免 sudo 化（D-Bus 路径）/ 监控历史曲线

**2026-08-30 十轮（下一批完成：dGPU+GPU模式页+电池健康，commits f7bc0a2/d84b23e/0d4ec40）**：
- backend `armoury_set` 白名单命令（nv_temp_target/nv_dynamic_boost/gpu_mux_mode/dgpu_disable；ppt 系排除走 set_power；socket 0666 场景防任意固件写）
- GUI 新增 **GPU 页**：MUX 三模式按钮（Eco/Hybrid/独显直连，组合写 dgpu_disable+gpu_mux_mode）、pending_reboot 橙色提示、dGPU 温度墙滑条(75-87°C)+动态加速滑条（初值对齐固件：实测 87°C/25W）、TGP 信息（实测 115W=RTX4060 规格）；无 nv 接口自动隐藏
- 电池页新增**健康组**：健康度%（energy_full/design，≥80% 绿色，实测 89.3%）、循环、电压、瞬时功率
- **install.sh 修复**：sudo 构建后 `chown -R $SUDO_USER target`（曾因 root 构建 target 污染 781 文件致用户侧构建 Permission denied）

## 10. 交接注意

- 改 bridge 后必须 `cargo build -p asustuner-gui` 验证（cxx-qt 宏报错信息晦涩，报 token 错通常指 bridge 文件第一行，真凶在内容语法）。
- 修改 QML 后必须重新编译（qml 打进 qrc）。
- 测试写操作后把系统状态恢复（如档位切回 quiet）。
- asusd 是系统服务，别动它的配置；我们只是客户端。
- 本文档随大改动更新；小改动更新 §4 状态表即可。

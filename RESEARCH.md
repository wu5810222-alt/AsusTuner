# 上游调研报告（供主对话裁决）

> 2026-08-30 · 侧 chat 调研汇总 · 结论已经过自我修正，剔除"预设找优点"的偏差
> 调研对象：OpenGamingCollective/asusctl（官方，本机 asusd 6.3.8 来源）、ixoz/z-helper、Ichihiroy/ghelper-for-linux、Intrusive-Thots/asus-kbd-rgb、Greifent/afc-gui

---

## 0. 一句话总结

**现有方案没有任何一条需要因本次调研而立即修改**。真正带走的：三个已验证事实、四个待实验点、一份竞品定位。最终方案需主对话对 §4 的问题清单裁决。

## 1. 竞品格局（定位判断）

| 项目 | 规模 | 与本项目的重叠 |
|---|---|---|
| **asusctl / rog-control-center**（官方，560★，2136 commits，活跃） | 大 | 后端(asusd)完全重叠——我们就是它的前端；GUI 的档位/风扇/充电/灯效/托盘功能重叠 |
| z-helper（3★，8 commits） | 极小 | 最像我们：GTK4 五页签、直写 sysfs、带 systemd 持久化（开机+60s 轮询+睡眠重断言） |
| ghelper-for-linux | 小 | TUI，电池上限为主，udev+systemd 持久化 |
| afc-gui（45★） | 小 | 仅风扇（基于 asus-fan-control，非 asusd） |
| asus-kbd-rgb 等零星 | ~0 | 仅键盘 RGB（同一 kbd_rgb_mode sysfs，佐证我们路线正确） |

**官方 rog-control-center 明确不做/不支持的**：ryzenadj 系（功率墙走 asus-armoury 内核接口，要求 Linux 6.19+；Curve Optimizer 降压不在计划）、G-Helper 式轻托盘快捷菜单（它的托盘只有 打开/退出/GPU状态图 三项）、X11、Debian/Ubuntu/Pop!_OS、拒绝纯 AI 贡献。

**建议定位（待确认）**：不做 rog-control-center 替代品；做「G-Helper 体验 + asusd 没有的功耗功能（ryzenadj 降压 / RAPL 监控 / 状态保持守护）」。

## 2. 已实测验证的事实（高置信度）

1. **本机内核 7.1.9-zen 有 asus-armoury 固件属性接口**（`/sys/class/firmware-attributes/asus-armoury/attributes/`）：
   - CPU 功率墙：`ppt_pl1_spl` / `ppt_pl2_sppt` / `ppt_pl3_fppt`（实测当前 100/115/135W，范围 30-135W，单位瓦）
   - dGPU：`nv_temp_target`(75-87°C)、`nv_dynamic_boost`、`nv_tgp`、`nv_base_tgp` —— ryzenadj 做不了的新能力
   - GPU 模式：`gpu_mux_mode` / `dgpu_disable`（不需要 supergfxd 的轻量路径）
   - `current_value` 普通用户**可读**；asusd 已包成 D-Bus（`/xyz/ljones/asus_armoury/ppt_pl1_spl`，读已验证）
   - **写入未测试**（见 §3-A）
2. **常驻成本实测**：root 后端 RSS 8.1MB / 空闲 CPU 0；对照 asusd 8.8MB；GUI(Qt Quick) RSS 323MB。后端加常驻功能预计 10-12MB。
3. 本机 RGB 单区键盘，固件仅 4 种效果——硬件上限，接口层无隐藏效果（多方案交叉印证）。

## 3. 候选改进项（按置信度分级，含代价）

| # | 项 | 置信度 | 收益 | 代价/风险 | 状态 |
|---|---|---|---|---|---|
| A | 功率墙迁 armoury（替代 ryzenadj） | **低**（只测过读） | 读回当前值+min/max 驱动 GUI 滑条 | 写入行为未知；可能与档位切换联动被 BIOS 重写；ryzenadj 已验证且是唯一降压路径 | **需先做写入实验**（§4-Q1） |
| B | dGPU 控制（nv_dynamic_boost / nv_tgp / nv_temp_target） | 高（接口存在） | G-Helper 同款 GPU 功耗功能，全新能力 | 同 A，写入未验证 | 待实验（§4-Q2） |
| C | RGB 写入经验：写后 sleep 0.15-0.2s（WMI 异步）+ 唤醒后二次重断言（z-helper 踩坑注释） | 中（他人注释） | 提高灯效/持久化可靠性 | 无 | 做持久化时直接采纳 |
| D | 电池健康页（循环数/健康度%/电压电流功率，ghelper-for-linux） | 高（标准 sysfs） | 监控页增强 | 无 | 低优先级可选 |
| E | ksni 纯 Rust 托盘 | **低**（绿地选择非结论） | 常驻托盘 ~2-5MB（vs GUI 藏托盘 323MB） | 多一套 IPC 与状态同步 | §4-Q4 |
| F | 持久化守护方案 | **关键修正** | 见下 | 见下 | §4-Q3 |

**F 的关键修正（最重要）**：我曾主张"事件驱动比轮询优雅"，这是错的——
- ppd **直写 sysfs 不走 asusd**，asusd 未必发属性变更信号；sysfs 的 inotify 不可靠 → 对"ppd 抢占档位"场景，事件驱动**可能根本收不到事件**。
- ryzenadj 唤醒后丢失也无信号可订阅，只能盲重放或轮询校验。
- **用户最初的 systemctl 脚本（sleep/resume 钩子 + 重应用）是实践检验的正解**，z-helper 的 60s 轮询同理（笨但什么都兜得住）。
- 可采信的混合形态：systemd sleep/resume 钩子（或 logind 信号）触发**盲重放** + 低频定时**校验档位**（轮询兜底）+ RGB 按 C 项经验处理。

## 4. 待主对话裁决的问题清单

- **Q1 功率墙是否做 armoury 写入实验？** 实验方案：写同值→写异值→`ryzenadj -i` 与 armoury 交叉对读→切档位后观察是否被重写。通过则功率墙 UI 换 armoury（ryzenadj 保留仅做降压）；不通过维持现状。倾向：值得做实验（成本半小时，读回+范围对 GUI 是真实提升）。
- **Q2 dGPU 控制（nv_dynamic_boost/nv_temp_target）是否纳入路线图？** 依赖 Q1 的写入实验结论。倾向：纳入（G-Helper 对标的核心缺口）。
- **Q3 持久化守护形态**：① systemd sleep 钩子+定时校验（用户脚本思路产品化，实践正解）② 纯事件驱动（已被修正为不可靠）③ 混合（钩子盲重放+低频轮询兜底）。倾向：③，且按 C 项经验处理 RGB。守护形态（常驻 root 后端 + systemd 单元）与 §2-2 的 8-12MB 成本已确认可接受。
- **Q4 托盘技术选型**：① ksni 独立轻托盘（2-5MB，+IPC 复杂度）② GUI 进程内托盘（简单，但常驻=323MB）③ 延后到守护做完再定。倾向：③ 先延后。
- **Q5 电池健康页优先级**：低成本展示功能，排期即可。
- **Q6 定位确认**：按 §1 建议定位执行（G-Helper 体验 + asusd 缺口），不做 rog-control-center 替代品；GPU 模式将来优先评估 armoury 的 `gpu_mux_mode`/`dgpu_disable`（比启用 supergfxd 轻）。

## 5. 明确不采纳（被修正的过度结论，防回潮）

- ❌ "armoury 碾压 ryzenadj，应迁移" —— 写入未验证，维持 ryzenadj 现状
- ❌ "事件驱动持久化更优雅" —— ppd 场景可能收不到事件，轮询/钩子不可耻
- ❌ "ksni 比其他托盘方案优" —— 绿地选择，无对照物，延后裁决
- ❌ 迁移顺序建议 —— 撤回，待裁决后再定

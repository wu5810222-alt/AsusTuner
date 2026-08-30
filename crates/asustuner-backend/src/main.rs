// AsusTuner 常驻 root 后端
//
// 架构（方案 B 第一层）：
//   - Unix socket JSON 行协议（多客户端：GUI / 托盘 / CLI 均可连）
//   - 状态持久化 /var/lib/asustuner/state.json：所有"设置型"命令写入即保存
//   - 启动时自动恢复已存状态（覆盖"重启被 BIOS 重置"）
//   - 事件自愈：logind 睡眠唤醒后重放（覆盖"睡眠重置"）；
//     asusd 档位被 ppd 等外部改动且开启"锁定档位"时自动夺回
//
// 协议（每行一条 JSON，应答同行返回）：
//   通用: ping / get_state / restore / set_lock_profile{on} / exit
//   设置(持久): set_profile{name} set_charge{limit} set_fan_curve{fan,temp,pwm}
//               fan_defaults set_power{stapm,fast,slow} set_curve{all_cores,igpu}
//               set_tctl{deg} boost{on} kbd{level} aura{mode,r,g,b,speed}
//   直通(不持久): ryzenadj{args} exec{args} rapl

use std::io::{BufRead, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod fan_curves;

// ---------- 状态 ----------

#[derive(Serialize, Deserialize, Clone, Default)]
struct AuraState {
    mode: u8,
    r: u8,
    g: u8,
    b: u8,
    speed: u8,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct State {
    profile: Option<String>,
    #[serde(default = "default_true")]
    profile_lock: bool,
    charge_limit: Option<u8>,
    stapm: Option<u32>,
    fast: Option<u32>,
    slow: Option<u32>,
    coall: Option<i32>,
    cogfx: Option<i32>,
    tctl: Option<u32>,
    boost: Option<bool>,
    kbd: Option<u8>,
    aura: Option<AuraState>,
    cpu_temp: Option<Vec<u8>>,
    cpu_pwm: Option<Vec<u8>>,
    gpu_temp: Option<Vec<u8>>,
    gpu_pwm: Option<Vec<u8>>,
}

fn default_true() -> bool {
    true
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(load_state()));

fn state_path() -> String {
    std::env::var("ASUSTUNER_STATE").unwrap_or_else(|_| "/var/lib/asustuner/state.json".into())
}

fn load_state() -> State {
    let p = state_path();
    match std::fs::read_to_string(&p) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(st) => {
                log(&format!("已加载状态 {p}"));
                st
            }
            Err(e) => {
                log(&format!("状态文件解析失败({e})，使用默认"));
                State::default()
            }
        },
        Err(_) => State::default(),
    }
}

fn save_state(st: &State) {
    let p = state_path();
    if let Some(dir) = std::path::Path::new(&p).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(st) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&p, s) {
                log(&format!("状态保存失败: {e}"));
            }
        }
        Err(e) => log(&format!("状态序列化失败: {e}")),
    }
}

fn log(msg: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "[backend] {msg}");
}

// ---------- 硬件操作 ----------

fn run_cmd(bin: &str, args: &[String]) -> (bool, String) {
    match std::process::Command::new(bin).args(args).output() {
        Ok(o) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            (o.status.success(), text.trim().to_string())
        }
        Err(e) => (false, format!("执行失败: {e}")),
    }
}

fn hwmon_fan_rpms() -> (i64, i64) {
    // 找 name=asus 的 hwmon，读 fan1_input/fan2_input (RPM)
    let dir = match std::fs::read_dir("/sys/class/hwmon") {
        Ok(d) => d,
        Err(_) => return (-1, -1),
    };
    for e in dir.flatten() {
        let d = e.path();
        let is_asus = std::fs::read_to_string(d.join("name"))
            .map(|n| n.trim() == "asus")
            .unwrap_or(false);
        if is_asus {
            let f1 = std::fs::read_to_string(d.join("fan1_input"))
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(-1);
            let f2 = std::fs::read_to_string(d.join("fan2_input"))
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(-1);
            return (f1, f2);
        }
    }
    (-1, -1)
}

fn write_sysfs(path: &str, val: &str) -> (bool, String) {
    match std::fs::write(path, val) {
        Ok(_) => (true, String::new()),
        Err(e) => (false, format!("写入 {path} 失败: {e}")),
    }
}

fn ryzenadj(args: Vec<String>) -> (bool, String) {
    run_cmd("/usr/sbin/ryzenadj", &args)
}

/// 功率墙写入：优先 asus-armoury 固件接口（mW→W 整数，实测可写、与档位解耦、
/// 不跨重启持久——依赖启动重放）；无 armoury 的机型回退 ryzenadj（mW 直写）。
/// 映射：stapm→PL1 / fast→PL3(FPPT) / slow→PL2(SPPT)。
fn write_power(stapm: u32, fast: u32, slow: u32) -> (bool, String) {
    let base = "/sys/class/firmware-attributes/asus-armoury/attributes";
    let pl1 = format!("{base}/ppt_pl1_spl/current_value");
    if !std::path::Path::new(&pl1).exists() {
        return ryzenadj(vec![
            format!("--stapm-limit={stapm}"),
            format!("--fast-limit={fast}"),
            format!("--slow-limit={slow}"),
        ]);
    }
    let (s, f, sl) = (stapm / 1000, fast / 1000, slow / 1000);
    let mut errs: Vec<String> = Vec::new();
    for (path, v, name) in [
        (format!("{base}/ppt_pl1_spl/current_value"), s, "PL1"),
        (format!("{base}/ppt_pl3_fppt/current_value"), f, "PL3"),
        (format!("{base}/ppt_pl2_sppt/current_value"), sl, "PL2"),
    ] {
        if let Err(e) = std::fs::write(&path, v.to_string()) {
            errs.push(format!("{name}: {e}"));
        }
        // 固件写入间隔（WMI/attr 异步生效）
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    if errs.is_empty() {
        (true, format!("armoury: PL1={s}W PL3={f}W PL2={sl}W"))
    } else {
        (false, errs.join("; "))
    }
}

fn platform_proxy() -> anyhow::Result<zbus::blocking::Proxy<'static>> {
    let conn = zbus::blocking::Connection::system()?;
    Ok(zbus::blocking::Proxy::new_owned(
        conn,
        "xyz.ljones.Asusd",
        "/xyz/ljones",
        "xyz.ljones.Platform",
    )?)
}

fn profile_val(name: &str) -> Option<u32> {
    match name.to_ascii_lowercase().as_str() {
        "balanced" => Some(0),
        "performance" | "turbo" => Some(1),
        "quiet" | "silent" => Some(2),
        "lowpower" => Some(3),
        _ => None,
    }
}

fn asusd_set_profile(name: &str) -> Result<(), String> {
    let val = profile_val(name).ok_or_else(|| "未知档位".to_string())?;
    let p = platform_proxy().map_err(|e| e.to_string())?;
    p.set_property::<u32>("PlatformProfile", val)
        .map_err(|e| e.to_string())
}

fn asusd_set_charge(limit: u8) -> Result<(), String> {
    let p = platform_proxy().map_err(|e| e.to_string())?;
    p.set_property::<u8>("ChargeControlEndThreshold", limit)
        .map_err(|e| e.to_string())
}

fn asusd_set_fan(fan: &str, temp: &[u8], pwm: &[u8]) -> Result<(), String> {
    use fan_curves::{write_fan_curve, CurveData, FanCurvePU};
    if temp.len() != 8 || pwm.len() != 8 {
        return Err("曲线需 8 个点".into());
    }
    let conn = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
    let proxy = fan_curves::FanCurvesProxyBlocking::new(&conn).map_err(|e| e.to_string())?;
    let p = platform_proxy().map_err(|e| e.to_string())?;
    let profile: u32 = proxy_current_profile(&p)?;
    let mut t = [0u8; 8];
    let mut w = [0u8; 8];
    t.copy_from_slice(temp);
    w.copy_from_slice(pwm);
    let curve = CurveData {
        fan: if fan == "gpu" { FanCurvePU::GPU } else { FanCurvePU::CPU },
        pwm: w,
        temp: t,
        enabled: true,
    };
    proxy
        .set_fan_curve(profile, curve)
        .map_err(|e| e.to_string())
}

fn proxy_current_profile(
    p: &zbus::blocking::Proxy<'static>,
) -> Result<u32, String> {
    p.get_property::<u32>("PlatformProfile").map_err(|e| e.to_string())
}

fn asusd_fan_defaults() -> Result<(), String> {
    use fan_curves::FanCurvesProxyBlocking;
    let conn = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
    let proxy = FanCurvesProxyBlocking::new(&conn).map_err(|e| e.to_string())?;
    let profile: u32 = {
        let p = platform_proxy().map_err(|e| e.to_string())?;
        proxy_current_profile(&p)?
    };
    proxy
        .set_curves_to_defaults(profile)
        .map_err(|e| e.to_string())
}

fn kbd_rgb_mode(mode: u8, r: u8, g: u8, b: u8, speed: u8) -> (bool, String) {
    write_sysfs(
        "/sys/class/leds/asus::kbd_backlight/kbd_rgb_mode",
        &format!("1 {mode} {r} {g} {b} {speed}"),
    )
}

// ---------- 状态应用 ----------

/// 恢复全部已存状态（启动时 / restore 命令 / 唤醒后）。
fn apply_state() {
    let st = STATE.lock().unwrap().clone();
    log("── 应用已保存状态 ──");

    if let Some(name) = &st.profile {
        // asusd 可能尚未就绪（开机竞态），重试两次
        let mut ok = asusd_set_profile(name);
        if ok.is_err() {
            std::thread::sleep(std::time::Duration::from_secs(3));
            ok = asusd_set_profile(name);
        }
        log(&format!("档位 {name}: {}", ok.is_ok()));
    }
    if let Some(on) = st.boost {
        let val = if on { "1" } else { "0" };
        let global = "/sys/devices/system/cpu/cpufreq/boost";
        let r = if std::path::Path::new(global).exists() {
            write_sysfs(global, val)
        } else {
            let mut last = (false, String::from("未找到 boost"));
            for i in 0..128 {
                let p = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/boost");
                if std::path::Path::new(&p).exists() {
                    last = write_sysfs(&p, val);
                }
            }
            last
        };
        log(&format!("boost={on}: {}", r.0));
    }
    if st.stapm.is_some() || st.fast.is_some() || st.slow.is_some() {
        let (ok, out) = write_power(
            st.stapm.unwrap_or(0),
            st.fast.unwrap_or(0),
            st.slow.unwrap_or(0),
        );
        log(&format!("功率墙: {ok} {out}"));
    }
    if let Some(v) = st.coall {
        if v != 0 {
            let (ok, out) = ryzenadj(vec![format!("--set-coall={v}")]);
            log(&format!("降压 coall={v}: {ok} {out}"));
        }
    }
    if let Some(v) = st.cogfx {
        if v != 0 {
            let (ok, out) = ryzenadj(vec![format!("--set-cogfx={v}")]);
            log(&format!("降压 cogfx={v}: {ok} {out}"));
        }
    }
    if let Some(d) = st.tctl {
        let (ok, out) = ryzenadj(vec![format!("--tctl-temp={d}")]);
        log(&format!("温度墙 {d}: {ok} {out}"));
    }
    if let (Some(t), Some(p)) = (&st.cpu_temp, &st.cpu_pwm) {
        let r = asusd_set_fan("cpu", t, p);
        log(&format!("CPU 风扇曲线: {}", r.is_ok()));
    }
    if let (Some(t), Some(p)) = (&st.gpu_temp, &st.gpu_pwm) {
        let r = asusd_set_fan("gpu", t, p);
        log(&format!("GPU 风扇曲线: {}", r.is_ok()));
    }
    if let Some(c) = st.charge_limit {
        let r = asusd_set_charge(c);
        log(&format!("充电限制 {c}: {}", r.is_ok()));
    }
    if let Some(l) = st.kbd {
        let r = write_sysfs(
            "/sys/class/leds/asus::kbd_backlight/brightness",
            &l.to_string(),
        );
        log(&format!("键盘亮度 {l}: {}", r.0));
        // WMI 写入异步生效，给固件落定时间（z-helper 踩坑经验）
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    if let Some(a) = st.aura {
        let r = kbd_rgb_mode(a.mode, a.r, a.g, a.b, a.speed);
        log(&format!("Aura mode={}: {}", a.mode, r.0));
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
}

// ---------- 命令处理 ----------

fn handle(v: Value) -> Value {
    let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let g32 = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let g64 = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);

    match cmd.as_str() {
        "ping" => json!({"ok": true, "msg": "pong"}),
        "get_state" => {
            let st = STATE.lock().unwrap().clone();
            json!({"ok": true, "state": st})
        }
        "restore" => {
            apply_state();
            json!({"ok": true})
        }
        "set_lock_profile" => {
            let on = v.get("on").and_then(|x| x.as_bool()).unwrap_or(true);
            {
                let mut st = STATE.lock().unwrap();
                st.profile_lock = on;
                save_state(&st);
            }
            log(&format!("锁定档位 = {on}"));
            json!({"ok": true})
        }
        "exit" => json!({"ok": true, "msg": "bye"}), // 服务模式：仅断开当前连接

        "set_profile" => {
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let r = asusd_set_profile(&name);
            if r.is_ok() {
                let mut st = STATE.lock().unwrap();
                st.profile = Some(name.clone());
                save_state(&st);
            }
            log(&format!("档位 → {name}: {}", r.is_ok()));
            json!({"ok": r.is_ok(), "out": r.err().unwrap_or_default()})
        }
        "set_charge" => {
            let limit = g64("limit") as u8;
            let r = asusd_set_charge(limit);
            if r.is_ok() {
                let mut st = STATE.lock().unwrap();
                st.charge_limit = Some(limit);
                save_state(&st);
            }
            json!({"ok": r.is_ok(), "out": r.err().unwrap_or_default()})
        }
        "set_fan_curve" => {
            let fan = v.get("fan").and_then(|x| x.as_str()).unwrap_or("cpu").to_string();
            let gv = |k: &str| -> Option<Vec<u8>> {
                v.get(k).and_then(|x| x.as_array()).map(|a| {
                    a.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect()
                })
            };
            let (Some(temp), Some(pwm)) = (gv("temp"), gv("pwm")) else {
                return json!({"ok": false, "out": "缺少 temp/pwm"});
            };
            let r = asusd_set_fan(&fan, &temp, &pwm);
            if r.is_ok() {
                let mut st = STATE.lock().unwrap();
                if fan == "gpu" {
                    st.gpu_temp = Some(temp);
                    st.gpu_pwm = Some(pwm);
                } else {
                    st.cpu_temp = Some(temp);
                    st.cpu_pwm = Some(pwm);
                }
                save_state(&st);
            }
            log(&format!("风扇曲线 {fan}: {}", r.is_ok()));
            json!({"ok": r.is_ok(), "out": r.err().unwrap_or_default()})
        }
        "fan_defaults" => {
            let r = asusd_fan_defaults();
            if r.is_ok() {
                let mut st = STATE.lock().unwrap();
                st.cpu_temp = None;
                st.cpu_pwm = None;
                st.gpu_temp = None;
                st.gpu_pwm = None;
                save_state(&st);
            }
            json!({"ok": r.is_ok(), "out": r.err().unwrap_or_default()})
        }
        "set_power" => {
            let (a, b, c) = (g32("stapm"), g32("fast"), g32("slow"));
            let (ok, out) = write_power(a, b, c);
            if ok {
                let mut st = STATE.lock().unwrap();
                st.stapm = Some(a);
                st.fast = Some(b);
                st.slow = Some(c);
                save_state(&st);
            }
            json!({"ok": ok, "out": out})
        }
        "set_curve" => {
            // 拆成独立调用：单项失败不掩盖另一项（如 Dragon Range 不支持 cogfx）
            let all = g64("all_cores") as i32;
            let igpu = g64("igpu") as i32;
            if all == 0 && igpu == 0 {
                return json!({"ok": false, "out": "无有效参数（0=不变更）"});
            }
            let mut ok_any = false;
            let mut detail: Vec<String> = Vec::new();
            if all != 0 {
                let (ok, out) = ryzenadj(vec![format!("--set-coall={all}")]);
                if ok {
                    ok_any = true;
                    detail.push(format!("coall={all}: 成功"));
                } else {
                    detail.push(format!("coall={all}: 失败 {out}"));
                }
            }
            if igpu != 0 {
                let (ok, out) = ryzenadj(vec![format!("--set-cogfx={igpu}")]);
                if ok {
                    ok_any = true;
                    detail.push(format!("cogfx={igpu}: 成功"));
                } else {
                    detail.push(format!("cogfx={igpu}: 不支持/失败 {out}"));
                }
            }
            if ok_any {
                let mut st = STATE.lock().unwrap();
                if all != 0 {
                    st.coall = Some(all);
                }
                // cogfx 仅在它自己成功时保存
                if igpu != 0 && detail.iter().any(|d| d.contains("cogfx=") && d.contains("成功")) {
                    st.cogfx = Some(igpu);
                }
                save_state(&st);
            }
            // 至少一项生效即 ok=true；单项失败明细在 out
            json!({"ok": ok_any, "out": detail.join(" | ")})
        }
        "set_tctl" => {
            let d = g32("deg");
            let (ok, out) = ryzenadj(vec![format!("--tctl-temp={d}")]);
            if ok {
                let mut st = STATE.lock().unwrap();
                st.tctl = Some(d);
                save_state(&st);
            }
            json!({"ok": ok, "out": out})
        }
        "boost" => {
            let on = v.get("on").and_then(|x| x.as_bool()).unwrap_or(false);
            let val = if on { "1" } else { "0" };
            let global = "/sys/devices/system/cpu/cpufreq/boost";
            let r = if std::path::Path::new(global).exists() {
                write_sysfs(global, val)
            } else {
                let mut last = (false, String::from("未找到 boost"));
                for i in 0..128 {
                    let p = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/boost");
                    if std::path::Path::new(&p).exists() {
                        last = write_sysfs(&p, val);
                    }
                }
                last
            };
            if r.0 {
                let mut st = STATE.lock().unwrap();
                st.boost = Some(on);
                save_state(&st);
            }
            json!({"ok": r.0, "out": r.1})
        }
        "kbd" => {
            let level = g64("level") as u8;
            let r = write_sysfs(
                "/sys/class/leds/asus::kbd_backlight/brightness",
                &level.to_string(),
            );
            if r.0 {
                let mut st = STATE.lock().unwrap();
                st.kbd = Some(level);
                save_state(&st);
            }
            json!({"ok": r.0, "out": r.1})
        }
        "armoury_set" => {
            // 固件属性白名单写入（dGPU/GPU 模式等；ppt 系走 set_power）
            const ALLOW: &[&str] = &[
                "nv_temp_target",
                "nv_dynamic_boost",
                "gpu_mux_mode",
                "dgpu_disable",
            ];
            let attr = v
                .get("attr")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if !ALLOW.contains(&attr.as_str()) {
                return json!({"ok": false, "out": format!("属性不在白名单: {attr}")});
            }
            let val = g64("value");
            let path = format!(
                "/sys/class/firmware-attributes/asus-armoury/attributes/{attr}/current_value"
            );
            let r = write_sysfs(&path, &val.to_string());
            log(&format!("armoury {attr}={val} -> {}", r.0));
            json!({"ok": r.0, "out": r.1})
        }
        "fan_calibrate" => {
            // 校准满转速：存当前曲线 → 全速 6s 采样峰值 → 恢复
            use fan_curves::{write_fan_curve, CurveData, FanCurvePU};
            let run = || -> Result<(i64, i64), String> {
                let conn = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
                let proxy = fan_curves::FanCurvesProxyBlocking::new(&conn).map_err(|e| e.to_string())?;
                let p = platform_proxy().map_err(|e| e.to_string())?;
                let profile = proxy_current_profile(&p)?;
                let saved = proxy
                    .fan_curve_data(profile)
                    .map_err(|e| e.to_string())?;
                log("校准：风扇全速运转约 6 秒…");
                let temps = [40u8, 50, 60, 70, 80, 90, 95, 100];
                for fan in [FanCurvePU::CPU, FanCurvePU::GPU] {
                    write_fan_curve(
                        profile,
                        CurveData { fan, pwm: [255u8; 8], temp: temps, enabled: true },
                    )
                    .map_err(|e| e.to_string())?;
                }
                let (mut m1, mut m2) = (0i64, 0i64);
                for _ in 0..15 {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    let (f1, f2) = hwmon_fan_rpms();
                    m1 = m1.max(f1);
                    m2 = m2.max(f2);
                }
                for c in &saved {
                    write_fan_curve(profile, c.clone()).map_err(|e| e.to_string())?;
                }
                log("校准完成，已恢复原曲线");
                Ok((m1, m2))
            };
            match run() {
                Ok((c, g)) => {
                    log(&format!("校准结果 CPU={c} GPU={g}"));
                    json!({"ok": c > 0 && g > 0, "calibrate": {"cpu": c, "gpu": g}})
                }
                Err(e) => {
                    log(&format!("校准失败: {e}"));
                    json!({"ok": false, "out": e.to_string()})
                }
            }
        }
        "aura" => {
            let g8 = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as u8;
            let (m, r_, g_, b_, s) = (g8("mode"), g8("r"), g8("g"), g8("b"), g8("speed"));
            let r = kbd_rgb_mode(m, r_, g_, b_, s);
            if r.0 {
                let mut st = STATE.lock().unwrap();
                st.aura = Some(AuraState { mode: m, r: r_, g: g_, b: b_, speed: s });
                save_state(&st);
            }
            json!({"ok": r.0, "out": r.1})
        }

        // 直通（不持久）
        "ryzenadj" => {
            let args: Vec<String> = v
                .get("args")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let (ok, out) = ryzenadj(args);
            json!({"ok": ok, "out": out})
        }
        "exec" => {
            let cmdline = v.get("args").and_then(|x| x.as_str()).unwrap_or("").to_string();
            log(&format!("exec: {cmdline}"));
            let (ok, out) = run_cmd("sh", &["-c".to_string(), cmdline]);
            json!({"ok": ok, "out": out})
        }
        "rapl" => {
            let read = |p: &str| -> i64 {
                std::fs::read_to_string(p)
                    .ok()
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .unwrap_or(-1)
            };
            let energy = read("/sys/class/powercap/intel-rapl:0/energy_uj");
            let range = read("/sys/class/powercap/intel-rapl:0/max_energy_range_uj");
            json!({"ok": energy >= 0, "energy_uj": energy, "range_uj": range})
        }
        other => json!({"ok": false, "error": format!("未知命令: {other}")}),
    }
}

// ---------- socket 服务 ----------

fn socket_path() -> String {
    std::env::var("ASUSTUNER_SOCKET").unwrap_or_else(|_| "/run/asustuner-backend.sock".into())
}

fn serve_socket() -> anyhow::Result<()> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    if let Ok(full) = std::fs::canonicalize(&path) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&full, std::fs::Permissions::from_mode(0o666));
    }
    log(&format!("socket 就绪: {path}"));
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                std::thread::spawn(move || {
                    let mut reader = std::io::BufReader::new(match s.try_clone() {
                        Ok(c) => c,
                        Err(_) => return,
                    });
                    let mut writer = s;
                    for line in reader.lines() {
                        let line = match line {
                            Ok(l) => l,
                            Err(_) => break,
                        };
                        if line.trim().is_empty() {
                            continue;
                        }
                        let reply = match serde_json::from_str::<Value>(&line) {
                            Ok(v) => handle(v),
                            Err(e) => json!({"ok": false, "error": format!("JSON 解析失败: {e}")}),
                        };
                        let _ = writeln!(writer, "{reply}");
                        let _ = writer.flush();
                    }
                });
            }
            Err(e) => log(&format!("accept 失败: {e}")),
        }
    }
    Ok(())
}

// ---------- 事件自愈 ----------

/// logind: 唤醒后重放状态。
fn watch_sleep() {
    std::thread::spawn(move || {
        let run = || -> anyhow::Result<()> {
            let conn = zbus::blocking::Connection::system()?;
            let p = zbus::blocking::Proxy::new_owned(
                conn,
                "org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
            )?;
            let iter = p.receive_signal("PrepareForSleep")?;
            for msg in iter {
                let going: bool = msg.body().deserialize().unwrap_or(false);
                if !going {
                    log("检测到唤醒，2 秒后重放状态…");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    apply_state();
                }
            }
            Ok(())
        };
        if let Err(e) = run() {
            log(&format!("logind 监听不可用: {e}"));
        }
    });
}

/// asusd: 档位被外部改动（ppd 等）且开启锁定时自动夺回。
fn watch_profile() {
    std::thread::spawn(move || {
        let run = || -> anyhow::Result<()> {
            let p = platform_proxy()?;
            let iter = p.receive_property_changed::<u32>("PlatformProfile");
            for _ in iter {
                std::thread::sleep(std::time::Duration::from_millis(300)); // 去抖
                let st = STATE.lock().unwrap().clone();
                let (Some(saved), true) = (&st.profile, st.profile_lock) else {
                    continue;
                };
                let cur = proxy_current_profile(&p).unwrap_or(u32::MAX);
                let want = profile_val(saved).unwrap_or(u32::MAX);
                if cur != want {
                    log(&format!("检测到档位被外部改为 {cur}，夺回为 {saved}"));
                    let _ = asusd_set_profile(saved);
                }
            }
            Ok(())
        };
        if let Err(e) = run() {
            log(&format!("asusd 档位监听不可用: {e}"));
        }
    });
}

/// 低频校验轮询（兜底腿）：ppd 等可能直写 sysfs 绕过 asusd 事件链路，
/// 定期比对当前档位与保存值，漂移才写回——笨但什么都兜得住。
fn verify_loop() {
    let secs: u64 = std::env::var("ASUSTUNER_VERIFY_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(45);
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(secs));
        let st = STATE.lock().unwrap().clone();
        let (Some(saved), true) = (&st.profile, st.profile_lock) else {
            continue;
        };
        let Ok(p) = platform_proxy() else {
            continue;
        };
        let cur = proxy_current_profile(&p).unwrap_or(u32::MAX);
        let want = profile_val(saved).unwrap_or(u32::MAX);
        if cur != want {
            log(&format!("校验轮询发现档位漂移（当前 {cur} ≠ 保存 {saved}），夺回"));
            let _ = asusd_set_profile(saved);
        }
    });
}

fn main() {
    // 单例守护（先于 apply_state，避免第二实例重复写硬件后互相夺回打架）
    if UnixStream::connect(socket_path()).is_ok() {
        log("检测到已有后端实例在运行，本实例退出");
        std::process::exit(0);
    }
    log("asustuner-backend 启动（常驻 socket 模式）");
    // 启动即恢复已保存状态（覆盖重启重置）
    apply_state();
    watch_sleep();
    watch_profile();
    verify_loop();
    if let Err(e) = serve_socket() {
        log(&format!("socket 服务失败: {e}"));
        std::process::exit(1);
    }
}

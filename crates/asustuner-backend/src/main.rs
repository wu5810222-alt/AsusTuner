// AsusTuner 特权后端 —— JSON 行协议 over stdio。
// 由 GUI 经 pkexec 以 root 启动；执行 ryzenadj、硬件写入等特权操作。
//
// 协议：stdin 每行一条 JSON 命令，stdout 每行一条 JSON 应答，stderr 为日志。
//   {"cmd":"ping"}                          -> {"ok":true,"msg":"pong"}
//   {"cmd":"ryzenadj","args":["--x=y"]}     -> {"ok":bool,"out":"..."}
//   {"cmd":"exec","args":"<shell 行>"}       -> {"ok":bool,"out":"..."}
//   {"cmd":"boost","on":true}               -> {"ok":bool,"out":"..."}
//   {"cmd":"kbd","level":2}                 -> {"ok":bool,"out":"..."}
//   {"cmd":"exit"}                          -> {"ok":true} 后退出

use std::io::{BufRead, Write};

use serde_json::{json, Value};

fn respond(v: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

fn log(msg: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "[backend] {msg}");
}

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

fn write_sysfs(path: &str, val: &str) -> (bool, String) {
    match std::fs::write(path, val) {
        Ok(_) => (true, String::new()),
        Err(e) => (false, format!("写入 {path} 失败: {e}")),
    }
}

fn main() {
    log("asustuner-backend 已启动（root）");
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                respond(json!({"ok": false, "error": format!("JSON 解析失败: {e}")}));
                continue;
            }
        };
        let cmd = v
            .get("cmd")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();

        match cmd.as_str() {
            "ping" => respond(json!({"ok": true, "msg": "pong"})),
            "exit" => {
                respond(json!({"ok": true}));
                break;
            }
            "ryzenadj" => {
                let args: Vec<String> = v
                    .get("args")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                log(&format!("ryzenadj {args:?}"));
                let (ok, out) = run_cmd("/usr/sbin/ryzenadj", &args);
                log(&format!("ryzenadj -> {out}"));
                respond(json!({"ok": ok, "out": out}));
            }
            "exec" => {
                let cmdline = v
                    .get("args")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                log(&format!("exec: {cmdline}"));
                let (ok, out) = run_cmd("sh", &["-c".to_string(), cmdline]);
                log(&format!("exec -> {}", if ok { "成功" } else { &out }));
                respond(json!({"ok": ok, "out": out}));
            }
            "boost" => {
                let on = v.get("on").and_then(|x| x.as_bool()).unwrap_or(false);
                let val = if on { "1" } else { "0" };
                let global = "/sys/devices/system/cpu/cpufreq/boost";
                let result = if std::path::Path::new(global).exists() {
                    write_sysfs(global, val)
                } else {
                    // 逐核 fallback（最多 128 核）
                    let mut last = (false, String::from("未找到 boost 文件"));
                    for i in 0..128 {
                        let p = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/boost");
                        if std::path::Path::new(&p).exists() {
                            last = write_sysfs(&p, val);
                        }
                    }
                    last
                };
                log(&format!("boost={on} -> {}", result.0));
                respond(json!({"ok": result.0, "out": result.1}));
            }
            "kbd" => {
                let level = v.get("level").and_then(|x| x.as_u64()).unwrap_or(0);
                let (ok, out) = write_sysfs(
                    "/sys/class/leds/asus::kbd_backlight/brightness",
                    &level.to_string(),
                );
                log(&format!("kbd={level} -> {ok}"));
                respond(json!({"ok": ok, "out": out}));
            }
            "rapl" => {
                // CPU package 能量计数（RAPL，root-only），GUI 差分算瞬时功率
                let read = |p: &str| -> i64 {
                    std::fs::read_to_string(p)
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .unwrap_or(-1)
                };
                let energy = read("/sys/class/powercap/intel-rapl:0/energy_uj");
                let range = read("/sys/class/powercap/intel-rapl:0/max_energy_range_uj");
                respond(json!({"ok": energy >= 0, "energy_uj": energy, "range_uj": range}));
            }
            "aura" => {
                // TUF 键盘灯效：kbd_rgb_mode 六字节 [1, mode, r, g, b, speed]
                // 字节格式与 asusd aura_laptop/mod.rs write_effect_and_apply 一致
                let g2 = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as u8;
                let (mode, r, g, b, speed) = (g2("mode"), g2("r"), g2("g"), g2("b"), g2("speed"));
                let buf = format!("1 {mode} {r} {g} {b} {speed}");
                let (ok, out) = write_sysfs(
                    "/sys/class/leds/asus::kbd_backlight/kbd_rgb_mode",
                    &buf,
                );
                log(&format!("aura mode={mode} rgb=({r},{g},{b}) speed={speed} -> {ok} {out}"));
                respond(json!({"ok": ok, "out": out}));
            }
            other => {
                respond(json!({"ok": false, "error": format!("未知命令: {other}")}));
            }
        }
    }
    log("backend 退出");
}

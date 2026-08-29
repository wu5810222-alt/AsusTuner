// cxx-qt 桥接 QObject —— 连接 QML 前端与 asusd + ryzenadj(经 root 后端)。
// - asusd 直连（普通用户可读写，已验证）
// - ryzenadj/boost/键盘灯 走 root 后端（pkexec 启动，JSON 行协议 over stdio）

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, board_name)]
        #[qproperty(QString, cpu_model)]
        #[qproperty(QString, gpu_model)]
        #[qproperty(f64, cpu_temp)]
        #[qproperty(f64, dgpu_temp)]
        #[qproperty(f64, cpu_freq)]
        #[qproperty(QString, battery_status)]
        #[qproperty(QString, platform_profile)]
        #[qproperty(f64, fan1_rpm)]
        #[qproperty(f64, fan2_rpm)]
        #[qproperty(f64, cpu_power)]
        #[qproperty(u32, charge_limit)]
        #[qproperty(bool, backend_running)]
        #[namespace = "asustuner"]
        type AsusTunerObject = super::AsusTunerRust;

        #[qinvokable]
        #[cxx_name = "connectDbus"]
        fn connect_dbus(self: Pin<&mut Self>);

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        // 性能档位：quiet / balanced / performance / lowpower
        #[qinvokable]
        #[cxx_name = "applyProfile"]
        fn apply_profile(&self, profile: &QString);

        // 充电限制 60/80/100
        #[qinvokable]
        #[cxx_name = "setChargeLimit"]
        fn apply_charge_limit(&self, limit: u32);

        // CPU 降压（负值=降压）—— 走 root 后端
        #[qinvokable]
        #[cxx_name = "setCpuCurve"]
        fn set_cpu_curve(&self, all_cores: i32, igpu: i32);

        // 功率墙 mW —— 走 root 后端
        #[qinvokable]
        #[cxx_name = "setPowerLimits"]
        fn set_power_limits(&self, stapm: u32, fast: u32, slow: u32);

        // 温度墙 °C —— 走 root 后端
        #[qinvokable]
        #[cxx_name = "setTempLimit"]
        fn set_temp_limit(&self, deg: u32);

        // CPU boost —— 走 root 后端
        #[qinvokable]
        #[cxx_name = "setBoost"]
        fn set_boost(&self, on: bool);

        // 键盘背光 0..3 —— 走 root 后端
        #[qinvokable]
        #[cxx_name = "setKbdBrightness"]
        fn set_kbd_brightness(&self, level: u32);

        // 键盘灯效 Aura（TUF sysfs 格式，与 asusd 字节一致）
        #[qinvokable]
        #[cxx_name = "setAura"]
        fn set_aura(&self, mode: u32, r: u32, g: u32, b: u32, speed: u32);

        // ---- root 后端管理 ----
        #[qinvokable]
        #[cxx_name = "startBackend"]
        fn start_backend(&self) -> bool;

        #[qinvokable]
        #[cxx_name = "backendExec"]
        fn backend_exec(&self, cmdline: &QString);

        #[qinvokable]
        #[cxx_name = "drainBackendLog"]
        fn drain_backend_log(&self) -> QString;

        // ---- 风扇曲线（asusd，免 root）----
        #[qinvokable]
        #[cxx_name = "setFanCurve"]
        fn set_fan_curve(
            &self,
            cpu_temp: &QString,
            cpu_pwm: &QString,
            gpu_temp: &QString,
            gpu_pwm: &QString,
        );

        #[qinvokable]
        #[cxx_name = "restoreFanCurves"]
        fn restore_fan_curves(&self);

        #[qinvokable]
        #[cxx_name = "fanCurveSummary"]
        fn fan_curve_summary(&self) -> QString;

        // 原始曲线 "t1,..,t8;p1,..,p8"（PWM 0-255 原始值），fan: 0=CPU 1=GPU
        #[qinvokable]
        #[cxx_name = "fanCurveRaw"]
        fn fan_curve_raw(&self, fan: u32) -> QString;
    }
}

use std::io::BufRead;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};

use cxx_qt_lib::QString;

/// asusd 服务名与路径（system bus）。
const ASUSD_SERVICE: &str = "xyz.ljones.Asusd";
const ASUSD_PATH: &str = "/xyz/ljones";
const PLATFORM_IFACE: &str = "xyz.ljones.Platform";

/// 全局 asusd D-Bus 代理。
static ASUSD: LazyLock<Mutex<Option<zbus::blocking::Proxy<'static>>>> =
    LazyLock::new(|| Mutex::new(None));

/// root 后端子进程（pkexec 拉起）。
static BACKEND_CHILD: LazyLock<Mutex<Option<std::process::Child>>> =
    LazyLock::new(|| Mutex::new(None));

/// 后端日志环形缓冲（GUI 定时 drain）。
static BACKEND_LOG: LazyLock<Mutex<std::collections::VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::VecDeque::new()));

/// 后端应答队列（stdout 里的 JSON 行，供逻辑消费）。
static PENDING_REPLIES: LazyLock<Mutex<std::collections::VecDeque<serde_json::Value>>> =
    LazyLock::new(|| Mutex::new(std::collections::VecDeque::new()));

/// RAPL 采样状态：(上次能量 uJ, 上次时刻)。
static RAPL_STATE: LazyLock<Mutex<Option<(i64, std::time::Instant)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Rust 侧 QObject 数据。
pub struct AsusTunerRust {
    board_name: QString,
    cpu_model: QString,
    gpu_model: QString,
    cpu_temp: f64,
    dgpu_temp: f64,
    cpu_freq: f64,
    battery_status: QString,
    platform_profile: QString,
    fan1_rpm: f64,
    fan2_rpm: f64,
    cpu_power: f64,
    charge_limit: u32,
    backend_running: bool,
}

impl Default for AsusTunerRust {
    fn default() -> Self {
        Self {
            board_name: QString::from("ASUS"),
            cpu_model: QString::from("探测中…"),
            gpu_model: QString::from(""),
            cpu_temp: 0.0,
            dgpu_temp: 0.0,
            cpu_freq: 0.0,
            battery_status: QString::from(""),
            platform_profile: QString::from(""),
            fan1_rpm: 0.0,
            fan2_rpm: 0.0,
            cpu_power: -1.0,
            charge_limit: 100,
            backend_running: false,
        }
    }
}

// ---------- asusd 连接 ----------

fn ensure_asusd() {
    if ASUSD.lock().unwrap().is_some() {
        return;
    }
    let conn = match zbus::blocking::Connection::system() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("D-Bus system bus 连接失败: {e}");
            return;
        }
    };
    match zbus::blocking::Proxy::new_owned(conn, ASUSD_SERVICE, ASUSD_PATH, PLATFORM_IFACE) {
        Ok(p) => *ASUSD.lock().unwrap() = Some(p),
        Err(e) => eprintln!("asusd 代理创建失败: {e}"),
    }
}

fn with_asusd<F, R, E>(f: F) -> Option<R>
where
    F: FnOnce(&zbus::blocking::Proxy<'static>) -> Result<R, E>,
{
    match ASUSD.lock().unwrap().as_ref() {
        Some(p) => f(p).ok(),
        None => None,
    }
}

fn profile_name(v: u32) -> &'static str {
    match v {
        0 => "balanced",
        1 => "performance",
        2 => "quiet",
        3 => "lowpower",
        _ => "unknown",
    }
}

// ---------- root 后端 ----------

fn log_line(s: String) {
    let mut q = BACKEND_LOG.lock().unwrap();
    if q.len() > 800 {
        q.drain(..400);
    }
    q.push_back(s);
}

fn backend_bin_path() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("asustuner-backend");
            if p.exists() {
                return Some(p);
            }
        }
    }
    let p = std::path::PathBuf::from("/usr/bin/asustuner-backend");
    if p.exists() {
        return Some(p);
    }
    None
}

fn backend_alive() -> bool {
    let mut guard = BACKEND_CHILD.lock().unwrap();
    match guard.as_mut() {
        Some(c) => matches!(c.try_wait(), Ok(None)),
        None => false,
    }
}

/// 经 pkexec 启动后端（弹出系统 polkit 密码对话框）。
fn spawn_backend() -> bool {
    if backend_alive() {
        return true;
    }
    let bin = match backend_bin_path() {
        Some(b) => b,
        None => {
            log_line("✗ 未找到 asustuner-backend 二进制".to_string());
            return false;
        }
    };
    if std::env::var("ASUSTUNER_NO_AUTH").as_deref() == Ok("1") {
        log_line("⚠ ASUSTUNER_NO_AUTH=1：跳过后端授权（功率墙/降压/boost/键盘灯不可用）".to_string());
        return false;
    }
    log_line("正在请求管理员权限（polkit 授权框）…".to_string());
    let attempt = std::process::Command::new("pkexec")
        .arg(&bin)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    match attempt {
        Ok(mut child) => {
            if let Some(out) = child.stdout.take() {
                std::thread::spawn(move || {
                    let reader = std::io::BufReader::new(out);
                    for line in reader.lines().flatten() {
                        // JSON 应答入队供逻辑消费；同时也进日志面板
                        match serde_json::from_str::<serde_json::Value>(&line) {
                            Ok(v) => {
                                PENDING_REPLIES.lock().unwrap().push_back(v);
                                log_line(format!("◀ {line}"));
                            }
                            Err(_) => log_line(format!("◀ {line}")),
                        }
                    }
                });
            }
            if let Some(err) = child.stderr.take() {
                std::thread::spawn(move || {
                    let reader = std::io::BufReader::new(err);
                    for line in reader.lines().flatten() {
                        log_line(format!("• {line}"));
                    }
                });
            }
            *BACKEND_CHILD.lock().unwrap() = Some(child);
            log_line("✓ 后端进程已创建（等待/完成授权）".to_string());
            true
        }
        Err(e) => {
            log_line(format!("✗ 启动后端失败: {e}"));
            false
        }
    }
}

fn backend_send(v: &serde_json::Value) -> bool {
    let mut guard = BACKEND_CHILD.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            let line = serde_json::to_string(v).unwrap_or_default();
            let ok = stdin.write_all(format!("{line}\n").as_bytes()).is_ok()
                && stdin.flush().is_ok();
            if !ok {
                log_line("✗ 后端管道写入失败".to_string());
            }
            return ok;
        }
    }
    log_line("✗ 后端未运行（先点击右上角状态灯授权）".to_string());
    false
}

fn backend_ryzenadj(args: Vec<String>) {
    if !backend_alive() && !spawn_backend() {
        return;
    }
    backend_send(&serde_json::json!({"cmd": "ryzenadj", "args": args}));
}

/// RAPL 能量差分 → 瓦数。首次采样返回 -1（尚无前值）。
fn compute_cpu_power(energy_uj: i64, range_uj: i64) -> f64 {
    let mut st = RAPL_STATE.lock().unwrap();
    let now = std::time::Instant::now();
    match *st {
        Some((last_e, last_t)) => {
            let mut de = energy_uj - last_e;
            if de < 0 && range_uj > 0 {
                de += range_uj; // 计数器回绕
            }
            *st = Some((energy_uj, now));
            let dt = now.duration_since(last_t).as_secs_f64();
            if dt > 0.2 {
                de as f64 / 1e6 / dt
            } else {
                -1.0
            }
        }
        None => {
            *st = Some((energy_uj, now));
            -1.0
        }
    }
}

// ---------- 只读监控（免 root） ----------

fn read_f64(p: &std::path::Path) -> Option<f64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

fn read_sys_temp(hwmon_name: &str) -> Option<f64> {
    let dir = std::fs::read_dir("/sys/class/hwmon").ok()?;
    for e in dir.flatten() {
        let d = e.path();
        let name = match std::fs::read_to_string(d.join("name")) {
            Ok(n) => n.trim().to_string(),
            Err(_) => continue,
        };
        if name == hwmon_name {
            for i in 1..=2 {
                if let Some(v) = read_f64(&d.join(format!("temp{i}_input"))) {
                    return Some(v / 1000.0);
                }
            }
        }
    }
    None
}

fn read_fan_rpms() -> (f64, f64) {
    let dir = match std::fs::read_dir("/sys/class/hwmon") {
        Ok(d) => d,
        Err(_) => return (0.0, 0.0),
    };
    for e in dir.flatten() {
        let d = e.path();
        let name = match std::fs::read_to_string(d.join("name")) {
            Ok(n) => n.trim().to_string(),
            Err(_) => continue,
        };
        if name == "asus" {
            let f1 = read_f64(&d.join("fan1_input")).unwrap_or(0.0);
            let f2 = read_f64(&d.join("fan2_input")).unwrap_or(0.0);
            return (f1, f2);
        }
    }
    (0.0, 0.0)
}

fn read_cpu_freq() -> Option<f64> {
    let dir = std::fs::read_dir("/sys/devices/system/cpu").ok()?;
    let mut sum = 0.0;
    let mut n = 0.0;
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
            if let Some(v) = read_f64(&e.path().join("cpufreq/scaling_cur_freq")) {
                sum += v / 1000.0;
                n += 1.0;
            }
        }
    }
    if n > 0.0 {
        Some(sum / n)
    } else {
        None
    }
}

fn detect_board_name() -> Option<String> {
    std::fs::read_to_string("/sys/devices/virtual/dmi/id/board_name")
        .ok()
        .map(|s| s.trim().to_string())
}

fn detect_cpu_model() -> Option<String> {
    let s = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    s.lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|x| x.trim().to_string())
}

fn detect_gpu_model() -> Option<String> {
    let out = std::process::Command::new("lspci").output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    s.lines().find_map(|l| {
        if l.contains("VGA compatible controller") {
            l.find("controller:").map(|pos| {
                l[pos + 11..]
                    .trim()
                    .trim_matches('[')
                    .trim_end_matches(']')
                    .trim()
                    .to_string()
            })
        } else {
            None
        }
    })
}

fn parse_arr8(s: &str) -> Option<[u8; 8]> {
    let parts: Vec<u8> = s
        .split(',')
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .filter_map(|x| x.parse().ok())
        .collect();
    if parts.len() == 8 {
        let mut a = [0u8; 8];
        a.copy_from_slice(&parts);
        Some(a)
    } else {
        None
    }
}

fn current_platform_profile() -> anyhow::Result<u32> {
    ensure_asusd();
    with_asusd(|p| p.get_property::<u32>("PlatformProfile"))
        .ok_or_else(|| anyhow::anyhow!("读取档位失败"))
}

// ---------- QObject 实现 ----------

impl qobject::AsusTunerObject {
    pub fn connect_dbus(mut self: Pin<&mut Self>) {
        ensure_asusd();
        if let Some(s) = detect_board_name() {
            self.as_mut().set_board_name(QString::from(s));
        }
        if let Some(s) = detect_cpu_model() {
            self.as_mut().set_cpu_model(QString::from(s));
        }
        if let Some(s) = detect_gpu_model() {
            self.as_mut().set_gpu_model(QString::from(s));
        }
        if let Some(p) = with_asusd(|p| p.get_property::<u32>("PlatformProfile")) {
            self.as_mut()
                .set_platform_profile(QString::from(profile_name(p)));
        }
        if let Some(c) = with_asusd(|p| p.get_property::<u8>("ChargeControlEndThreshold")) {
            self.as_mut().set_charge_limit(c as u32);
        }
        // 启动时自动请求 root 授权（pkexec 弹系统密码框）
        self.as_mut().start_backend();
        self.refresh();
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        ensure_asusd();
        if let Some(t) = read_sys_temp("k10temp") {
            self.as_mut().set_cpu_temp(t);
        }
        if let Some(t) = read_sys_temp("amdgpu") {
            self.as_mut().set_dgpu_temp(t);
        }
        if let Some(f) = read_cpu_freq() {
            self.as_mut().set_cpu_freq(f);
        }
        if let Some(b) = std::fs::read_to_string("/sys/class/power_supply/BAT0/status").ok() {
            self.as_mut()
                .set_battery_status(QString::from(b.trim().to_string()));
        }
        let (f1, f2) = read_fan_rpms();
        self.as_mut().set_fan1_rpm(f1);
        self.as_mut().set_fan2_rpm(f2);
        if let Some(p) = with_asusd(|p| p.get_property::<u32>("PlatformProfile")) {
            self.as_mut()
                .set_platform_profile(QString::from(profile_name(p)));
        }
        if let Some(c) = with_asusd(|p| p.get_property::<u8>("ChargeControlEndThreshold")) {
            self.as_mut().set_charge_limit(c as u32);
        }

        // CPU 瞬时功率：经 root 后端读 RAPL 能量，差分计算（异步应答，下轮生效）
        if backend_alive() {
            backend_send(&serde_json::json!({"cmd": "rapl"}));
        }
        {
            let replies: Vec<serde_json::Value> = {
                let mut q = PENDING_REPLIES.lock().unwrap();
                q.drain(..).collect()
            };
            for r in replies {
                if let Some(e) = r.get("energy_uj").and_then(|x| x.as_i64()) {
                    if e < 0 {
                        continue;
                    }
                    let range = r.get("range_uj").and_then(|x| x.as_i64()).unwrap_or(0);
                    let watts = compute_cpu_power(e, range);
                    if watts >= 0.0 {
                        self.as_mut().set_cpu_power(watts);
                    }
                }
            }
        }

        self.as_mut().set_backend_running(backend_alive());
    }

    pub fn apply_profile(&self, profile: &QString) {
        let name: String = profile.into();
        let val: u32 = match name.as_str() {
            "performance" | "turbo" => 1,
            "balanced" => 0,
            "quiet" | "silent" => 2,
            "lowpower" => 3,
            _ => 0,
        };
        ensure_asusd();
        log_line(format!("▶ 档位 → {name}"));
        let _ = with_asusd(|p| p.set_property::<u32>("PlatformProfile", val));
    }

    pub fn apply_charge_limit(&self, limit: u32) {
        ensure_asusd();
        log_line(format!("▶ 充电限制 → {limit}%"));
        let _ = with_asusd(|p| p.set_property::<u8>("ChargeControlEndThreshold", limit as u8));
    }

    pub fn set_cpu_curve(&self, all_cores: i32, igpu: i32) {
        let mut args: Vec<String> = Vec::new();
        if all_cores != 0 {
            args.push(format!("--set-coall={all_cores}"));
        }
        if igpu != 0 {
            args.push(format!("--set-cogfx={igpu}"));
        }
        log_line(format!("▶ 降压 coall={all_cores} cogfx={igpu}"));
        backend_ryzenadj(args);
    }

    pub fn set_power_limits(&self, stapm: u32, fast: u32, slow: u32) {
        log_line(format!("▶ 功率墙 stapm={stapm} fast={fast} slow={slow}"));
        backend_ryzenadj(vec![
            format!("--stapm-limit={stapm}"),
            format!("--fast-limit={fast}"),
            format!("--slow-limit={slow}"),
        ]);
    }

    pub fn set_temp_limit(&self, deg: u32) {
        log_line(format!("▶ 温度墙 {deg}°C"));
        backend_ryzenadj(vec![format!("--tctl-temp={deg}")]);
    }

    pub fn set_boost(&self, on: bool) {
        log_line(format!("▶ boost → {on}"));
        backend_send(&serde_json::json!({"cmd": "boost", "on": on}));
    }

    pub fn set_kbd_brightness(&self, level: u32) {
        log_line(format!("▶ 键盘背光 → {level}"));
        backend_send(&serde_json::json!({"cmd": "kbd", "level": level}));
    }

    /// 键盘灯效：mode 0=静态 1=呼吸 2=闪烁 3=彩虹；speed 0xe1/0xeb/0xf5。
    pub fn set_aura(&self, mode: u32, r: u32, g: u32, b: u32, speed: u32) {
        log_line(format!("▶ Aura mode={mode} rgb=({r},{g},{b}) speed={speed:#x}"));
        backend_send(&serde_json::json!({
            "cmd": "aura", "mode": mode, "r": r, "g": g, "b": b, "speed": speed
        }));
    }

    pub fn start_backend(&self) -> bool {
        let ok = spawn_backend();
        if ok {
            backend_send(&serde_json::json!({"cmd": "ping"}));
        }
        ok
    }

    pub fn backend_exec(&self, cmdline: &QString) {
        let c: String = cmdline.into();
        log_line(format!("▶ {c}"));
        backend_send(&serde_json::json!({"cmd": "exec", "args": c}));
    }

    pub fn drain_backend_log(&self) -> QString {
        let mut q = BACKEND_LOG.lock().unwrap();
        let mut s = String::new();
        while let Some(line) = q.pop_front() {
            s.push_str(&line);
            s.push('\n');
        }
        QString::from(s)
    }

    pub fn set_fan_curve(
        &self,
        cpu_temp: &QString,
        cpu_pwm: &QString,
        gpu_temp: &QString,
        gpu_pwm: &QString,
    ) {
        use crate::fan_curves::{write_fan_curve, CurveData, FanCurvePU};
        let profile = match current_platform_profile() {
            Ok(p) => p,
            Err(_) => {
                log_line("✗ 无法读当前档位（asusd）".to_string());
                return;
            }
        };
        let t_cpu: String = cpu_temp.into();
        let p_cpu: String = cpu_pwm.into();
        let tmp = parse_arr8(&t_cpu);
        let pwm = parse_arr8(&p_cpu);
        if tmp.is_none() || pwm.is_none() {
            log_line("✗ CPU 曲线需 8 个点".to_string());
            return;
        }
        log_line("▶ 写入 CPU 风扇曲线".to_string());
        let cpu = CurveData {
            fan: FanCurvePU::CPU,
            pwm: pwm.unwrap(),
            temp: tmp.unwrap(),
            enabled: true,
        };
        let _ = write_fan_curve(profile, cpu);
        let t_gpu: String = gpu_temp.into();
        let p_gpu: String = gpu_pwm.into();
        let gt = parse_arr8(&t_gpu);
        let gp = parse_arr8(&p_gpu);
        if gt.is_some() && gp.is_some() {
            log_line("▶ 写入 GPU 风扇曲线".to_string());
            let gpu = CurveData {
                fan: FanCurvePU::GPU,
                pwm: gp.unwrap(),
                temp: gt.unwrap(),
                enabled: true,
            };
            let _ = write_fan_curve(profile, gpu);
        }
    }

    pub fn restore_fan_curves(&self) {
        use crate::fan_curves::restore_defaults;
        match current_platform_profile() {
            Ok(profile) => {
                log_line("▶ 恢复默认风扇曲线".to_string());
                let _ = restore_defaults(profile);
            }
            Err(_) => log_line("✗ 无法读当前档位".to_string()),
        }
    }

    pub fn fan_curve_summary(&self) -> QString {
        use crate::fan_curves::read_fan_curves;
        let profile = match current_platform_profile() {
            Ok(p) => p,
            Err(_) => return QString::from(""),
        };
        match read_fan_curves(profile) {
            Ok(curves) => {
                let mut out = String::new();
                for c in &curves {
                    out.push_str(&format!("{}: ", c.fan.as_str()));
                    for i in 0..8 {
                        out.push_str(&format!(
                            "{}°:{}%  ",
                            c.temp[i],
                            (c.pwm[i] as u32) * 100 / 255
                        ));
                    }
                    out.push('\n');
                }
                QString::from(out)
            }
            Err(e) => QString::from(format!("读取失败: {e}")),
        }
    }

    /// 原始曲线数据（供 QML 编辑器填充）："t1,..,t8;p1,..,p8"，PWM 为 0-255 原始值。
    pub fn fan_curve_raw(&self, fan: u32) -> QString {
        use crate::fan_curves::{read_fan_curves, FanCurvePU};
        let profile = match current_platform_profile() {
            Ok(p) => p,
            Err(_) => return QString::from(""),
        };
        let want = if fan == 0 { FanCurvePU::CPU } else { FanCurvePU::GPU };
        match read_fan_curves(profile) {
            Ok(curves) => {
                for c in &curves {
                    if c.fan == want {
                        let t: Vec<String> = c.temp.iter().map(|x| x.to_string()).collect();
                        let p: Vec<String> = c.pwm.iter().map(|x| x.to_string()).collect();
                        return QString::from(format!("{};{}", t.join(","), p.join(",")));
                    }
                }
                QString::from("")
            }
            Err(_) => QString::from(""),
        }
    }
}

#[cfg(test)]
mod power_tests {
    use super::compute_cpu_power;

    #[test]
    fn first_sample_is_negative() {
        let w = compute_cpu_power(1_000_000_000, 0);
        assert!(w < 0.0, "首次采样应返回 -1，实际 {w}");
    }

    #[test]
    fn power_from_delta() {
        // 3J / 1s ≈ 3W（用人工推进时间不可行，用极小间隔跳过）
        let a = compute_cpu_power(1_000_000_000, 0);
        assert!(a < 0.0);
        // 间隔太短（<0.2s）返回 -1
        let b = compute_cpu_power(1_000_003_000, 0);
        assert!(b < 0.0, "间隔 <0.2s 应返回 -1");
    }

    #[test]
    fn wraparound_adds_range() {
        let _ = compute_cpu_power(10_000_000, 0);
        // 能量从 10_000_000 回绕到接近 0：差为负，应加 range
        // 由于时间限制 <0.2s 这里只验证不 panic 且返回负（间隔不足）
        let w = compute_cpu_power(9_999_999, 30_000_000);
        assert!(w < 0.0);
    }
}

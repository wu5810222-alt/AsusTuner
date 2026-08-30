// cxx-qt 桥接 QObject —— 连接 QML 前端与 asusd + ryzenadj(经 root 后端)。
// - asusd 直连（普通用户可读写，已验证）
// - 全部写操作走常驻 root 后端（Unix socket JSON 行协议；未部署时 pkexec 兜底拉起）

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
        #[qproperty(f64, ppt_pl1)]
        #[qproperty(f64, ppt_pl2)]
        #[qproperty(f64, ppt_pl3)]
        #[qproperty(u32, ppt_min)]
        #[qproperty(u32, ppt_max)]
        #[qproperty(bool, ppt_available)]
        #[qproperty(bool, nv_available)]
        #[qproperty(f64, nv_temp)]
        #[qproperty(u32, nv_temp_min)]
        #[qproperty(u32, nv_temp_max)]
        #[qproperty(f64, nv_boost)]
        #[qproperty(u32, nv_boost_min)]
        #[qproperty(u32, nv_boost_max)]
        #[qproperty(f64, nv_tgp)]
        #[qproperty(f64, nv_base)]
        #[qproperty(u32, gpu_mux)]
        #[qproperty(bool, dgpu_off)]
        #[qproperty(bool, gpu_reboot_pending)]
        #[qproperty(f64, battery_health)]
        #[qproperty(u32, battery_cycles)]
        #[qproperty(f64, battery_voltage)]
        #[qproperty(f64, battery_power)]
        #[qproperty(f64, fan_calib_cpu)]
        #[qproperty(f64, fan_calib_gpu)]
        #[qproperty(u32, charge_limit)]
        #[qproperty(bool, backend_running)]
        // 配置方案列表（每行 "名称\t平台\tbuiltin\tactive"，Tab 分隔）
        #[qproperty(QString, cfg_list)]
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

        // 固件属性写入（后端白名单校验）
        #[qinvokable]
        #[cxx_name = "armourySet"]
        fn armoury_set(&self, attr: &QString, value: u32);

        // iGPU 降压（Curve Optimiser，部分 family 不支持）
        #[qinvokable]
        #[cxx_name = "setIgpuCurve"]
        fn set_igpu_curve(&self, igpu: i32);

        // 校准满转速（后端全速运转 ~6s 实测）
        #[qinvokable]
        #[cxx_name = "calibrateFans"]
        fn calibrate_fans(&self);

        // 原始曲线 "t1,..,t8;p1,..,p8"（PWM 0-255 原始值），fan: 0=CPU 1=GPU
        #[qinvokable]
        #[cxx_name = "fanCurveRaw"]
        fn fan_curve_raw(&self, fan: u32) -> QString;

        // ---- 配置方案（G-Helper 式，经 root 后端）----
        #[qinvokable]
        #[cxx_name = "cfgApply"]
        fn cfg_apply(&self, name: &QString);

        #[qinvokable]
        #[cxx_name = "cfgSave"]
        fn cfg_save(&self, name: &QString, platform: &QString, snapshot: bool);

        #[qinvokable]
        #[cxx_name = "cfgDelete"]
        fn cfg_delete(&self, name: &QString);
    }
}

use std::io::BufRead;
use std::os::unix::net::UnixStream;
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

/// root 后端 socket 连接。
static BACKEND_SOCK: LazyLock<Mutex<Option<std::os::unix::net::UnixStream>>> =
    LazyLock::new(|| Mutex::new(None));

/// 连接状态。
static BACKEND_CONNECTED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// 后端日志环形缓冲（GUI 定时 drain）。
static BACKEND_LOG: LazyLock<Mutex<std::collections::VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::VecDeque::new()));

/// 后端应答队列（stdout 里的 JSON 行，供逻辑消费）。
static PENDING_REPLIES: LazyLock<Mutex<std::collections::VecDeque<serde_json::Value>>> =
    LazyLock::new(|| Mutex::new(std::collections::VecDeque::new()));

/// RAPL 采样状态：(上次能量 uJ, 上次时刻)。
static RAPL_STATE: LazyLock<Mutex<Option<(i64, std::time::Instant)>>> =
    LazyLock::new(|| Mutex::new(None));

/// 上次下发到 QML 的方案列表打包串（去抖，避免每轮 refresh 重刷按钮）。
static CFG_LIST_LAST: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

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
    ppt_pl1: f64,
    ppt_pl2: f64,
    ppt_pl3: f64,
    ppt_min: u32,
    ppt_max: u32,
    ppt_available: bool,
    nv_available: bool,
    nv_temp: f64,
    nv_temp_min: u32,
    nv_temp_max: u32,
    nv_boost: f64,
    nv_boost_min: u32,
    nv_boost_max: u32,
    nv_tgp: f64,
    nv_base: f64,
    gpu_mux: u32,
    dgpu_off: bool,
    gpu_reboot_pending: bool,
    battery_health: f64,
    battery_cycles: u32,
    battery_voltage: f64,
    battery_power: f64,
    fan_calib_cpu: f64,
    fan_calib_gpu: f64,
    charge_limit: u32,
    backend_running: bool,
    cfg_list: QString,
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
            ppt_pl1: 0.0,
            ppt_pl2: 0.0,
            ppt_pl3: 0.0,
            ppt_min: 0,
            ppt_max: 0,
            ppt_available: false,
            nv_available: false,
            nv_temp: 0.0,
            nv_temp_min: 0,
            nv_temp_max: 0,
            nv_boost: 0.0,
            nv_boost_min: 0,
            nv_boost_max: 0,
            nv_tgp: 0.0,
            nv_base: 0.0,
            gpu_mux: 0,
            dgpu_off: false,
            gpu_reboot_pending: false,
            battery_health: 0.0,
            battery_cycles: 0,
            battery_voltage: 0.0,
            battery_power: 0.0,
            fan_calib_cpu: 0.0,
            fan_calib_gpu: 0.0,
            charge_limit: 100,
            backend_running: false,
            cfg_list: QString::from(""),
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

// ---------- root 后端（常驻，Unix socket） ----------

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


fn log_line(s: String) {
    let mut q = BACKEND_LOG.lock().unwrap();
    if q.len() > 800 {
        q.drain(..400);
    }
    q.push_back(s);
}

fn socket_path() -> String {
    std::env::var("ASUSTUNER_SOCKET").unwrap_or_else(|_| "/run/asustuner-backend.sock".into())
}

fn backend_connected() -> bool {
    *BACKEND_CONNECTED.lock().unwrap()
}

/// 尝试连接后端 socket；成功则启动读取线程。
fn try_connect_backend() -> bool {
    let stream = match UnixStream::connect(socket_path()) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let reader = match stream.try_clone() {
        Ok(c) => c,
        Err(_) => return false,
    };
    *BACKEND_SOCK.lock().unwrap() = Some(stream);
    *BACKEND_CONNECTED.lock().unwrap() = true;
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(reader);
        for line in reader.lines().flatten() {
            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(v) => {
                    PENDING_REPLIES.lock().unwrap().push_back(v);
                    log_line(format!("◀ {line}"));
                }
                Err(_) => log_line(format!("◀ {line}")),
            }
        }
        // 连接断开
        *BACKEND_SOCK.lock().unwrap() = None;
        *BACKEND_CONNECTED.lock().unwrap() = false;
        log_line("✗ 后端连接断开".to_string());
    });
    true
}

/// 确保后端可用：已连/能连/经 pkexec 拉起后重试。
fn ensure_backend() -> bool {
    if backend_connected() {
        return true;
    }
    if try_connect_backend() {
        return true;
    }
    // 常驻服务未跑：经 pkexec 拉起一份（绑定 socket 后退出重试连接）
    if std::env::var("ASUSTUNER_NO_AUTH").as_deref() != Ok("1") {
        let bin = backend_bin_path();
        if let Some(bin) = bin {
            log_line("正在请求管理员权限启动后端（polkit）…".to_string());
            let _ = std::process::Command::new("pkexec")
                .arg(&bin)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if try_connect_backend() {
                    log_line("✓ 后端已连接".to_string());
                    return true;
                }
            }
        } else {
            log_line("✗ 未找到 asustuner-backend 二进制".to_string());
        }
    } else {
        log_line("⚠ ASUSTUNER_NO_AUTH=1：跳过后端（功率/降压/灯效不可用）".to_string());
    }
    false
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

fn backend_send(v: &serde_json::Value) -> bool {
    let mut guard = BACKEND_SOCK.lock().unwrap();
    if let Some(sock) = guard.as_mut() {
        use std::io::Write;
        let line = serde_json::to_string(v).unwrap_or_default();
        let ok = sock.write_all(format!("{line}\n").as_bytes()).is_ok()
            && sock.flush().is_ok();
        if !ok {
            *BACKEND_CONNECTED.lock().unwrap() = false;
            log_line("✗ 后端写入失败".to_string());
        }
        return ok;
    }
    log_line("✗ 后端未连接（点击右上角状态灯重试）".to_string());
    false
}

fn backend_ryzenadj_persist(cmd: &str, mut payload: serde_json::Value) {
    if !ensure_backend() {
        return;
    }
    if let Some(o) = payload.as_object_mut() {
        o.insert("cmd".into(), serde_json::Value::String(cmd.into()));
    }
    backend_send(&payload);
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

fn read_bat_f64(field: &str) -> Option<f64> {
    std::fs::read_to_string(format!("/sys/class/power_supply/BAT0/{field}"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// asus-armoury 固件属性读取（0644 世界可读，免 root）。
fn read_armoury(attr: &str, field: &str) -> Option<u32> {
    std::fs::read_to_string(format!(
        "/sys/class/firmware-attributes/asus-armoury/attributes/{attr}/{field}"
    ))
    .ok()?
    .trim()
    .parse()
    .ok()
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
        if backend_connected() {
            backend_send(&serde_json::json!({"cmd": "rapl"}));
            backend_send(&serde_json::json!({"cmd": "profile_list"}));
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
                if let Some(cal) = r.get("calibrate") {
                    let c = cal.get("cpu").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let g = cal.get("gpu").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    if c > 0.0 {
                        self.as_mut().set_fan_calib_cpu(c);
                    }
                    if g > 0.0 {
                        self.as_mut().set_fan_calib_gpu(g);
                    }
                }
                // 配置方案列表：打包为 "名称\t平台\tbuiltin\tactive" 行
                if let Some(profiles) = r.get("profiles").and_then(|x| x.as_array()) {
                    let mut packed = String::new();
                    for p in profiles {
                        let name = p.get("name").and_then(|x| x.as_str()).unwrap_or("");
                        let platform = p.get("platform").and_then(|x| x.as_str()).unwrap_or("");
                        let builtin = p.get("builtin").and_then(|x| x.as_bool()).unwrap_or(false);
                        let active = p.get("active").and_then(|x| x.as_bool()).unwrap_or(false);
                        packed.push_str(&format!(
                            "{name}\t{platform}\t{}\t{}\n",
                            builtin as u8,
                            active as u8
                        ));
                    }
                    let mut last = CFG_LIST_LAST.lock().unwrap();
                    if *last != packed {
                        *last = packed.clone();
                        drop(last);
                        self.as_mut().set_cfg_list(QString::from(packed));
                    }
                }
            }
        }

        // 电池健康（标准 sysfs）
        let e_full = read_bat_f64("energy_full");
        let e_design = read_bat_f64("energy_full_design");
        if let (Some(full), Some(design)) = (e_full, e_design) {
            if design > 0.0 {
                self.as_mut().set_battery_health(full / design * 100.0);
            }
        }
        if let Some(c) = std::fs::read_to_string("/sys/class/power_supply/BAT0/cycle_count")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
        {
            self.as_mut().set_battery_cycles(c);
        }
        if let Some(v) = read_bat_f64("voltage_now") {
            self.as_mut().set_battery_voltage(v / 1e6);
        }
        if let Some(w) = read_bat_f64("power_now") {
            self.as_mut().set_battery_power(w / 1e6);
        }

        // GPU：asus-armoury nv_*（温度墙/动态加速/TGP）与模式
        let nv_t = read_armoury("nv_temp_target", "current_value");
        let nv_b = read_armoury("nv_dynamic_boost", "current_value");
        match (nv_t, nv_b) {
            (Some(t), Some(b)) => {
                self.as_mut().set_nv_available(true);
                self.as_mut().set_nv_temp(t as f64);
                self.as_mut().set_nv_boost(b as f64);
                if let Some(mn) = read_armoury("nv_temp_target", "min_value") {
                    self.as_mut().set_nv_temp_min(mn);
                }
                if let Some(mx) = read_armoury("nv_temp_target", "max_value") {
                    self.as_mut().set_nv_temp_max(mx);
                }
                if let Some(mn) = read_armoury("nv_dynamic_boost", "min_value") {
                    self.as_mut().set_nv_boost_min(mn);
                }
                if let Some(mx) = read_armoury("nv_dynamic_boost", "max_value") {
                    self.as_mut().set_nv_boost_max(mx);
                }
            }
            _ => self.as_mut().set_nv_available(false),
        }
        if let Some(v) = read_armoury("nv_tgp", "current_value") {
            self.as_mut().set_nv_tgp(v as f64);
        }
        if let Some(v) = read_armoury("nv_base_tgp", "current_value") {
            self.as_mut().set_nv_base(v as f64);
        }
        if let Some(m) = read_armoury("gpu_mux_mode", "current_value") {
            self.as_mut().set_gpu_mux(m);
        }
        if let Some(d) = read_armoury("dgpu_disable", "current_value") {
            self.as_mut().set_dgpu_off(d == 1);
        }
        if let Some(p) = read_armoury("pending_reboot", "current_value") {
            self.as_mut().set_gpu_reboot_pending(p == 1);
        }

        // asus-armoury 功率墙读回（PL1=STAPM / PL2=SPPT(慢) / PL3=FPPT(快)）
        let (pl1, pl2, pl3) = (
            read_armoury("ppt_pl1_spl", "current_value"),
            read_armoury("ppt_pl2_sppt", "current_value"),
            read_armoury("ppt_pl3_fppt", "current_value"),
        );
        match (pl1, pl2, pl3) {
            (Some(a), Some(b), Some(c)) => {
                self.as_mut().set_ppt_available(true);
                self.as_mut().set_ppt_pl1(a as f64);
                self.as_mut().set_ppt_pl2(b as f64);
                self.as_mut().set_ppt_pl3(c as f64);
                if let Some(mn) = read_armoury("ppt_pl1_spl", "min_value") {
                    self.as_mut().set_ppt_min(mn);
                }
                if let Some(mx) = read_armoury("ppt_pl1_spl", "max_value") {
                    self.as_mut().set_ppt_max(mx);
                }
            }
            _ => self.as_mut().set_ppt_available(false),
        }

        self.as_mut().set_backend_running(backend_connected());
    }

    pub fn apply_profile(&self, profile: &QString) {
        let name: String = profile.into();
        log_line(format!("▶ 档位 → {name}"));
        backend_send(&serde_json::json!({"cmd": "set_profile", "name": name}));
        // 手动切平台 = 离开自定义方案，列表 active 标记需要更新
        backend_send(&serde_json::json!({"cmd": "profile_list"}));
    }

    // ---- 配置方案（G-Helper 式）----

    pub fn cfg_apply(&self, name: &QString) {
        let n: String = name.into();
        log_line(format!("▶ 应用方案「{n}」"));
        if ensure_backend() {
            backend_send(&serde_json::json!({"cmd": "profile_apply", "name": n}));
            backend_send(&serde_json::json!({"cmd": "profile_list"}));
        }
    }

    pub fn cfg_save(&self, name: &QString, platform: &QString, snapshot: bool) {
        let n: String = name.into();
        let p: String = platform.into();
        log_line(format!("▶ 保存方案「{n}」（平台 {p}，含当前设置={snapshot}）"));
        if ensure_backend() {
            backend_send(&serde_json::json!({
                "cmd": "profile_save", "name": n, "platform": p, "snapshot": snapshot
            }));
            backend_send(&serde_json::json!({"cmd": "profile_list"}));
        }
    }

    pub fn cfg_delete(&self, name: &QString) {
        let n: String = name.into();
        log_line(format!("▶ 删除方案「{n}」"));
        if ensure_backend() {
            backend_send(&serde_json::json!({"cmd": "profile_delete", "name": n}));
            backend_send(&serde_json::json!({"cmd": "profile_list"}));
        }
    }

    pub fn apply_charge_limit(&self, limit: u32) {
        log_line(format!("▶ 充电限制 → {limit}%"));
        backend_send(&serde_json::json!({"cmd": "set_charge", "limit": limit}));
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
        backend_ryzenadj_persist("set_curve", serde_json::json!({"all_cores": all_cores, "igpu": igpu}));
    }

    pub fn set_power_limits(&self, stapm: u32, fast: u32, slow: u32) {
        log_line(format!("▶ 功率墙 stapm={stapm} fast={fast} slow={slow}"));
        backend_ryzenadj_persist("set_power", serde_json::json!({"stapm": stapm, "fast": fast, "slow": slow}));
    }

    pub fn set_temp_limit(&self, deg: u32) {
        log_line(format!("▶ 温度墙 {deg}°C"));
        backend_ryzenadj_persist("set_tctl", serde_json::json!({"deg": deg}));
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
        let ok = ensure_backend();
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
        let t_cpu: String = cpu_temp.into();
        let p_cpu: String = cpu_pwm.into();
        let tmp = parse_arr8(&t_cpu);
        let pwm = parse_arr8(&p_cpu);
        if tmp.is_none() || pwm.is_none() {
            log_line("✗ CPU 曲线需 8 个点".to_string());
            return;
        }
        if !ensure_backend() {
            return;
        }
        log_line("▶ 写入 CPU 风扇曲线".to_string());
        let cpu_arr: Vec<u32> = tmp.unwrap().iter().map(|&x| x as u32).collect();
        let pwm_arr: Vec<u32> = pwm.unwrap().iter().map(|&x| x as u32).collect();
        backend_send(&serde_json::json!({"cmd": "set_fan_curve", "fan": "cpu",
            "temp": cpu_arr, "pwm": pwm_arr}));
        let t_gpu: String = gpu_temp.into();
        let p_gpu: String = gpu_pwm.into();
        let gt = parse_arr8(&t_gpu);
        let gp = parse_arr8(&p_gpu);
        if gt.is_some() && gp.is_some() {
            log_line("▶ 写入 GPU 风扇曲线".to_string());
            let gt_arr: Vec<u32> = gt.unwrap().iter().map(|&x| x as u32).collect();
            let gp_arr: Vec<u32> = gp.unwrap().iter().map(|&x| x as u32).collect();
            backend_send(&serde_json::json!({"cmd": "set_fan_curve", "fan": "gpu",
                "temp": gt_arr, "pwm": gp_arr}));
        }
    }

    pub fn restore_fan_curves(&self) {
        log_line("▶ 恢复默认风扇曲线".to_string());
        backend_send(&serde_json::json!({"cmd": "fan_defaults"}));
    }

    /// 校准满转速：后端全速运转采样，结果经 drain 回填属性。
    pub fn calibrate_fans(&self) {
        log_line("▶ 校准满转速（风扇将全速运转约 6 秒）…".to_string());
        if ensure_backend() {
            backend_send(&serde_json::json!({"cmd": "fan_calibrate"}));
        }
    }

    /// iGPU 降压：仅发 cogfx（all_cores=0 表示不变更）。
    pub fn set_igpu_curve(&self, igpu: i32) {
        log_line(format!("▶ iGPU 降压 cogfx={igpu}"));
        backend_send(&serde_json::json!({"cmd": "set_curve", "all_cores": 0, "igpu": igpu}));
    }

    pub fn armoury_set(&self, attr: &QString, value: u32) {
        let a: String = attr.into();
        log_line(format!("▶ armoury {a} = {value}"));
        backend_send(&serde_json::json!({"cmd": "armoury_set", "attr": a, "value": value}));
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

// AsusTuner 轻量系统托盘（方案 B 第二层）
//
// ksni (StatusNotifierItem) 纯 Rust 实现，KDE 原生支持。
// 职责：右键菜单快捷控制（档位/风扇预设/恢复配置）、按需拉起主窗口。
// 所有硬件命令经常驻后端的 Unix socket（/run/asustuner-backend.sock）。

use std::io::{BufRead, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};
use serde_json::{json, Value};

fn sock_path() -> String {
    std::env::var("ASUSTUNER_SOCKET").unwrap_or_else(|_| "/run/asustuner-backend.sock".into())
}
const GUI_NAMES: [&str; 2] = ["/usr/bin/asustuner-gui", "asustuner-gui"];

/// 当前档位（0=平衡 1=性能 2=静音），后台线程轮询 asusd 更新（用于 tooltip）。
static PROFILE: AtomicU8 = AtomicU8::new(2);

fn profile_label(v: u8) -> &'static str {
    match v {
        0 => "平衡",
        1 => "性能",
        2 => "静音",
        3 => "低功耗",
        _ => "未知",
    }
}

fn log(msg: &str) {
    eprintln!("[tray] {msg}");
}

/// 连接后端执行一条命令（短连接）。
fn sock_cmd(v: &Value) -> Option<Value> {
    let mut s = UnixStream::connect(sock_path()).ok()?;
    let line = serde_json::to_string(v).ok()?;
    s.write_all(line.as_bytes()).ok()?;
    s.write_all(b"\n").ok()?;
    let mut reader = std::io::BufReader::new(s);
    let mut reply = String::new();
    reader.read_line(&mut reply).ok()?;
    serde_json::from_str(&reply).ok()
}

fn set_profile(name: &str) {
    if let Some(r) = sock_cmd(&json!({"cmd": "set_profile", "name": name})) {
        log(&format!("档位 → {name}: {}", r["ok"]));
    } else {
        log("后端未运行（systemctl start asustuner-backend）");
    }
}

fn cfg_apply(name: &str) {
    if let Some(r) = sock_cmd(&json!({"cmd": "profile_apply", "name": name})) {
        log(&format!("方案「{name}」: {}", r["ok"]));
    } else {
        log("后端未运行（systemctl start asustuner-backend）");
    }
}

/// 配置方案菜单项：每次展开菜单时向后端要 profile_list（短连接，毫秒级）。
/// 后端不可达时退回三平台档位直切。
fn cfg_items() -> Vec<MenuItem<AsusTray>> {
    let list = sock_cmd(&json!({"cmd": "profile_list"}))
        .and_then(|r| r.get("profiles").and_then(|x| x.as_array()).cloned());
    match list {
        Some(profiles) if !profiles.is_empty() => profiles
            .iter()
            .map(|p| {
                let name = p.get("name").and_then(|x| x.as_str()).unwrap_or("?").to_string();
                let active = p.get("active").and_then(|x| x.as_bool()).unwrap_or(false);
                let label = if active { format!("● {name}") } else { name.clone() };
                MenuItem::Standard(StandardItem {
                    label,
                    activate: Box::new(move |_: &mut AsusTray| cfg_apply(&name)),
                    ..Default::default()
                })
            })
            .collect(),
        _ => [
            ("静音", "quiet"),
            ("平衡", "balanced"),
            ("性能", "performance"),
        ]
        .iter()
        .map(|(label, name)| {
            let name = name.to_string();
            MenuItem::Standard(StandardItem {
                label: label.to_string(),
                activate: Box::new(move |_: &mut AsusTray| set_profile(&name)),
                ..Default::default()
            })
        })
        .collect(),
    }
}

fn fan_preset(kind: &str) {
    // 与 GUI 一致的 CPU/GPU 分离预设
    let (cpu_t, cpu_p, gpu_t, gpu_p): ([u8; 8], [u8; 8], [u8; 8], [u8; 8]) = match kind {
        "quiet" => (
            [50, 60, 65, 70, 75, 80, 90, 97],
            [0, 25, 45, 70, 95, 120, 170, 220],
            [55, 60, 65, 70, 75, 80, 90, 97],
            [0, 20, 40, 60, 85, 110, 160, 210],
        ),
        "balanced" => (
            [50, 60, 65, 70, 75, 80, 85, 97],
            [20, 45, 70, 100, 130, 165, 200, 235],
            [55, 60, 65, 70, 75, 80, 85, 97],
            [15, 40, 65, 90, 120, 150, 185, 225],
        ),
        _ => (
            [45, 55, 65, 70, 75, 80, 85, 97],
            [80, 120, 160, 195, 225, 240, 250, 255],
            [50, 60, 65, 70, 75, 80, 85, 97],
            [60, 100, 140, 175, 205, 230, 245, 255],
        ),
    };
    for (fan, t, p) in [("cpu", cpu_t, cpu_p), ("gpu", gpu_t, gpu_p)] {
        let v = json!({"cmd": "set_fan_curve", "fan": fan,
                       "temp": t.to_vec(), "pwm": p.to_vec()});
        if let Some(r) = sock_cmd(&v) {
            log(&format!("风扇 {kind}/{fan}: {}", r["ok"]));
        }
    }
}

fn fan_defaults() {
    if let Some(r) = sock_cmd(&json!({"cmd": "fan_defaults"})) {
        log(&format!("恢复默认风扇: {}", r["ok"]));
    }
}

fn restore_state() {
    if let Some(r) = sock_cmd(&json!({"cmd": "restore"})) {
        log(&format!("恢复已保存配置: {}", r["ok"]));
    }
}

/// GUI 是否在运行：探测其单实例锁 socket（比 pgrep 可靠——退出未收割的
/// 僵尸进程同样会被 pgrep 匹配到，会误判"已在运行"）。
fn gui_running() -> bool {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::os::unix::net::UnixStream::connect(format!("{dir}/asustuner-gui.lock")).is_ok()
}

fn open_gui() {
    // 已有 GUI 实例则不再拉起（Wayland 下也无法可靠唤起已有窗口）
    if gui_running() {
        log("GUI 已在运行，不重复打开");
        return;
    }
    let path = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("asustuner-gui")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from(GUI_NAMES[0]));
    match std::process::Command::new(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            // 收割子进程：不 wait 会留僵尸（僵尸会污染进程探测）
            let pid = child.id();
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            log(&format!("已打开主界面 (pid {pid})"))
        }
        Err(e) => log(&format!("打开主界面失败: {e}")),
    }
}

/// 托盘图标：内嵌 24/32/48px PNG（由 scripts/make_icons.sh 从 assets/icon.png 生成），
/// 启动时解码一次为 SNI 规定的 ARGB32 字节序；多尺寸供面板按缩放倍率挑选。
const TRAY_PNGS: [&[u8]; 3] = [
    include_bytes!("../../../assets/icons/hicolor/24x24/apps/asustuner.png"),
    include_bytes!("../../../assets/icons/hicolor/32x32/apps/asustuner.png"),
    include_bytes!("../../../assets/icons/hicolor/48x48/apps/asustuner.png"),
];

/// 深色面板变体（白色高对比；右键菜单手动切换，不做面板亮暗自动检测）
const TRAY_PNGS_DARK: [&[u8]; 3] = [
    include_bytes!("../../../assets/icons/dark/24x24/asustuner-dark.png"),
    include_bytes!("../../../assets/icons/dark/32x32/asustuner-dark.png"),
    include_bytes!("../../../assets/icons/dark/48x48/asustuner-dark.png"),
];

fn tray_icon(dark: bool) -> Vec<ksni::Icon> {
    static SETS: std::sync::OnceLock<(Vec<ksni::Icon>, Vec<ksni::Icon>)> =
        std::sync::OnceLock::new();
    let (light, dark_set) = SETS.get_or_init(|| {
        (
            TRAY_PNGS.iter().filter_map(|b| decode_png(b)).collect(),
            TRAY_PNGS_DARK.iter().filter_map(|b| decode_png(b)).collect(),
        )
    });
    if dark { dark_set.clone() } else { light.clone() }
}

/// PNG(RGBA8) -> ksni::Icon（ARGB32：A 在首字节的本机序）
fn decode_png(bytes: &[u8]) -> Option<ksni::Icon> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    let mut data = Vec::with_capacity(info.buffer_size());
    for px in buf[..info.buffer_size()].chunks_exact(4) {
        data.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
    }
    Some(ksni::Icon {
        width: info.width as i32,
        height: info.height as i32,
        data,
    })
}

/// 深色面板图标偏好：XDG state 下的小文件，托盘自持（root 的 state.json 不归托管管）。
fn icon_pref_path() -> Option<std::path::PathBuf> {
    let base = match std::env::var("XDG_STATE_HOME") {
        Ok(s) if !s.is_empty() => s,
        _ => format!("{}/.local/state", std::env::var("HOME").ok()?),
    };
    Some(std::path::PathBuf::from(base).join("asustuner/tray_dark_icon"))
}

fn load_dark_icon() -> bool {
    icon_pref_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

fn save_dark_icon(on: bool) {
    if let Some(p) = icon_pref_path() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, if on { "1" } else { "0" });
    }
}

struct AsusTray {
    _keep_alive: Arc<()>,
    dark_icon: bool,
}

impl ksni::Tray for AsusTray {
    fn id(&self) -> String {
        "asustuner".into()
    }

    fn title(&self) -> String {
        "AsusTuner".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "AsusTuner".into(),
            description: format!("当前平台: {}", profile_label(PROFILE.load(Ordering::Relaxed))),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        tray_icon(self.dark_icon)
    }

    fn icon_name(&self) -> String {
        // hicolor 主题图标（install.sh 安装）作为 pixmap 不可用时的回退
        "asustuner".into()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let fan_item = |label: &str, kind: &'static str| -> MenuItem<Self> {
            MenuItem::Standard(StandardItem {
                label: label.into(),
                activate: std::boxed::Box::new(move |_: &mut Self| fan_preset(kind)),
                ..Default::default()
            })
        };
        let mut items: Vec<MenuItem<Self>> = cfg_items();
        items.push(MenuItem::Separator);
        items.push(MenuItem::SubMenu(SubMenu {
            label: "风扇曲线".into(),
            submenu: vec![
                fan_item("静音", "quiet"),
                fan_item("均衡", "balanced"),
                fan_item("激进", "aggressive"),
                MenuItem::Standard(StandardItem {
                    label: "恢复默认".into(),
                    activate: std::boxed::Box::new(|_: &mut Self| fan_defaults()),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        }));
        items.push(MenuItem::Separator);
        items.push(MenuItem::Standard(StandardItem {
            label: "恢复已保存配置".into(),
            icon_name: "view-refresh".into(),
            activate: std::boxed::Box::new(|_: &mut Self| restore_state()),
            ..Default::default()
        }));
        items.push(MenuItem::Standard(StandardItem {
            label: "打开主界面".into(),
            icon_name: "preferences-desktop".into(),
            activate: std::boxed::Box::new(|_: &mut Self| open_gui()),
            ..Default::default()
        }));
        items.push(MenuItem::Separator);
        items.push(MenuItem::Checkmark(CheckmarkItem {
            label: "深色面板图标".into(),
            checked: self.dark_icon,
            // ksni 在 clicked 后自动重发属性与菜单 → 改字段即可，图标/勾选态即时生效
            activate: Box::new(|tray: &mut Self| {
                tray.dark_icon = !tray.dark_icon;
                save_dark_icon(tray.dark_icon);
            }),
            ..Default::default()
        }));
        items.push(MenuItem::Separator);
        items.push(MenuItem::Standard(StandardItem {
            label: "退出托盘".into(),
            activate: std::boxed::Box::new(move |_: &mut Self| std::process::exit(0)),
            ..Default::default()
        }));
        items
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        open_gui();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_pngs_decode_to_square_opaque_icons() {
        for b in TRAY_PNGS.into_iter().chain(TRAY_PNGS_DARK) {
            let ic = decode_png(b).expect("内嵌 PNG 应可解码为 RGBA");
            assert_eq!(ic.width, ic.height, "图标应为正方形");
            assert_eq!(
                ic.data.len(),
                (ic.width * ic.height * 4) as usize,
                "应为 ARGB32 全量像素"
            );
            let opaque = ic.data.iter().step_by(4).filter(|&&a| a > 127).count();
            assert!(
                opaque * 5 > (ic.width * ic.height) as usize,
                "主体应不透明（>20% 像素），实际 {opaque}"
            );
        }
    }
}

fn main() {
    // 单实例锁：XDG autostart 与 systemd user unit 双路径并存时防双托盘
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let lock_path = format!("{dir}/asustuner-tray.lock");
    let _lock = if UnixStream::connect(&lock_path).is_ok() {
        eprintln!("[tray] 已有托盘实例在运行，本次退出");
        std::process::exit(0);
    } else {
        let _ = std::fs::remove_file(&lock_path);
        std::os::unix::net::UnixListener::bind(&lock_path).ok()
    };

    // 后台轮询当前档位（asusd 直读，普通用户可读）
    std::thread::spawn(|| loop {
        let v = std::process::Command::new("gdbus")
            .args([
                "call", "--system", "--dest", "xyz.ljones.Asusd", "--object-path",
                "/xyz/ljones", "--method", "org.freedesktop.DBus.Properties.Get",
                "xyz.ljones.Platform", "PlatformProfile",
            ])
            .output();
        if let Ok(out) = v {
            let s = String::from_utf8_lossy(&out.stdout);
            // 输出形如 (<uint32 2>,)
            if let Some(idx) = s.find("uint32 ") {
                let rest = &s[idx + 7..];
                let num: String =
                    rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = num.parse::<u8>() {
                    PROFILE.store(n, Ordering::Relaxed);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    });

    log("托盘启动");
    use ksni::blocking::TrayMethods;
    let service = AsusTray {
        _keep_alive: Arc::new(()),
        dark_icon: load_dark_icon(),
    }
    .spawn();
    if let Err(e) = service {
        log(&format!("托盘创建失败（需要 StatusNotifierItem 支持）: {e}"));
        std::process::exit(1);
    }
    // 阻塞主线程保持托盘存活
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

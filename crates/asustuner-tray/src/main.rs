// AsusTuner 轻量系统托盘（方案 B 第二层）
//
// ksni (StatusNotifierItem) 纯 Rust 实现，KDE 原生支持。
// 职责：右键菜单快捷控制（档位/风扇预设/恢复配置）、按需拉起主窗口。
// 所有硬件命令经常驻后端的 Unix socket（/run/asustuner-backend.sock）。

use std::io::{BufRead, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use ksni::menu::{MenuItem, RadioItem, StandardItem, SubMenu};
use serde_json::{json, Value};

fn sock_path() -> String {
    std::env::var("ASUSTUNER_SOCKET").unwrap_or_else(|_| "/run/asustuner-backend.sock".into())
}
const GUI_NAMES: [&str; 2] = ["/usr/bin/asustuner-gui", "asustuner-gui"];

/// 当前档位（0=平衡 1=性能 2=静音），后台线程轮询 asusd 更新。
static PROFILE: AtomicU8 = AtomicU8::new(2);

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

fn open_gui() {
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
        Ok(_) => log("已打开主界面"),
        Err(e) => log(&format!("打开主界面失败: {e}")),
    }
}

/// 22x22 ARGB 图标：蓝色圆角方块 + 白色圆点。
fn tray_icon() -> Vec<ksni::Icon> {
    const N: usize = 22;
    let mut data = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let i = (y * N + x) * 4;
            // 圆角判定
            let corner = (x < 3 || x >= N - 3) && (y < 3 || y >= N - 3);
            let dx = x as i32 - 10;
            let dy = y as i32 - 10;
            let center = dx * dx + dy * dy <= 20; // 中心圆点半径 ~4.5
            data[i] = 255; // A
            if center {
                data[i + 1] = 255; // R
                data[i + 2] = 255; // G
                data[i + 3] = 255; // B
            } else if !corner {
                data[i + 1] = 0x29;
                data[i + 2] = 0x80;
                data[i + 3] = 0xb9;
            }
        }
    }
    vec![ksni::Icon { width: N as i32, height: N as i32, data }]
}

struct AsusTray {
    _keep_alive: Arc<()>,
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
            description: "系统控制：右键菜单操作".into(),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        tray_icon()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        use ksni::menu::{RadioGroup, SubMenu};
        let cur = PROFILE.load(Ordering::Relaxed);
        // selected 索引：0=静音(2) 1=平衡(0) 2=性能(1)
        let selected = match cur {
            0 => 1,
            1 => 2,
            _ => 0,
        };
        let fan_item = |label: &str, kind: &'static str| -> MenuItem<Self> {
            MenuItem::Standard(StandardItem {
                label: label.into(),
                activate: std::boxed::Box::new(move |_: &mut Self| fan_preset(kind)),
                ..Default::default()
            })
        };
        vec![
            MenuItem::RadioGroup(RadioGroup {
                selected,
                select: std::boxed::Box::new(|_: &mut Self, idx: usize| {
                    let (name, val) = match idx {
                        0 => ("quiet", 2u8),
                        1 => ("balanced", 0u8),
                        _ => ("performance", 1u8),
                    };
                    set_profile(name);
                    PROFILE.store(val, Ordering::Relaxed);
                }),
                options: vec![
                    RadioItem { label: "静音".into(), ..Default::default() },
                    RadioItem { label: "平衡".into(), ..Default::default() },
                    RadioItem { label: "性能".into(), ..Default::default() },
                ],
            }),
            MenuItem::Separator,
            MenuItem::SubMenu(SubMenu {
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
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "恢复已保存配置".into(),
                icon_name: "view-refresh".into(),
                activate: std::boxed::Box::new(|_: &mut Self| restore_state()),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: "打开主界面".into(),
                icon_name: "preferences-desktop".into(),
                activate: std::boxed::Box::new(|_: &mut Self| open_gui()),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "退出托盘".into(),
                activate: std::boxed::Box::new(|_: &mut Self| std::process::exit(0)),
                ..Default::default()
            }),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        open_gui();
    }
}

fn main() {
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

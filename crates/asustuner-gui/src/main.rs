//! AsusTuner GUI 入口 —— QML (cxx-qt) KDE 前端。

pub mod cxxqt_object;
pub mod fan_curves;

use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QUrl};
use std::pin::Pin;

/// Qt 资源里的 QML 入口路径，由 build.rs 的 `QmlModule::new("org.guts.AsusTuner.app")` 生成。
const QML_PATH: &str = "qrc:/qt/qml/org/guts/AsusTuner/app/qml/main.qml";

fn main() {
    // 单实例保护：锁 socket 可连接 = 已有实例，本次启动退出
    // （托盘重复点击/重复启动只保留第一个窗口）
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let lock_path = format!("{dir}/asustuner-gui.lock");
    let existing = std::os::unix::net::UnixStream::connect(&lock_path).is_ok();
    let _lock = if existing {
        eprintln!("asustuner-gui 已有实例在运行，本次启动退出");
        std::process::exit(0);
    } else {
        let _ = std::fs::remove_file(&lock_path);
        std::os::unix::net::UnixListener::bind(&lock_path).ok()
    };

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(QML_PATH));
    }

    if let Some(engine) = engine.as_mut() {
        let engine: Pin<&mut QQmlEngine> = engine.upcast_pin();
        engine.on_quit(|_| {
            println!("QML 已退出");
        }).release();
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}

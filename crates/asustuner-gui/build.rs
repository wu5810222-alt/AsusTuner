//! 构建脚本：生成 C++ 桥接并打包 QML 资源。
//!
//! 参照 cxx-qt 官方 `cargo_without_cmake` 示例：
//! - `new_qml_module` 声明 QML 模块（URI 会决定 qrc 资源路径）
//! - `qml_file` 指定 QML 入口，渲染进 Qt 资源
//! - `files` 列出 cxx bridge 定义的 Rust 源文件

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.guts.AsusTuner.app").qml_file("qml/main.qml"),
    )
    // Qt Qml 在 macOS 上需链接 Network；Linux 上无害，保持一致
    .qt_module("Network")
    // 注册 cxx-qt bridge 定义的 QObject
    .files(["src/cxxqt_object.rs"])
    .build();
}

#!/usr/bin/env bash
# AsusTuner 部署（方案 B 三件套）—— sudo 执行一次
#
#   1. 编译 release
#   2. 安装 backend / tray / gui / cli 到 /usr/bin
#   3. 启用 asustuner-backend.service（root 常驻：状态持久化 + 睡眠/档位事件自愈）
#   4. 托盘加入桌面自启动
#
# 之后：托盘常驻（右键菜单），主窗口从托盘按需打开。

set -euo pipefail

CYAN=$'\033[36m'; GREEN=$'\033[32m'; RED=$'\033[31m'; RESET=$'\033[0m'
info(){ echo "${CYAN}[AsusTuner]${RESET} $*"; }
ok(){ echo "${GREEN}[AsusTuner]${RESET} $*"; }
die(){ echo "${RED}[AsusTuner]${RESET} $*" >&2; exit 1; }

[[ "$(id -u)" -eq 0 ]] || die "请用 sudo 运行：sudo ./install.sh"
cd "$(dirname "$0")"

info "编译 release..."
cargo build --release || die "编译失败"
ok "编译完成"

info "安装二进制到 /usr/bin"
install -m 755 target/release/asustuner-backend /usr/bin/asustuner-backend
install -m 755 target/release/asustuner-tray    /usr/bin/asustuner-tray
install -m 755 target/release/asustuner-gui     /usr/bin/asustuner-gui
install -m 755 target/release/asustuner-cli     /usr/bin/asustuner-cli
ok "二进制已安装"

info "启用 root 后端服务（常驻）"
install -m 644 systemd/asustuner-backend.service /etc/systemd/system/asustuner-backend.service
systemctl daemon-reload
systemctl enable --now asustuner-backend.service
sleep 1
systemctl is-active asustuner-backend.service >/dev/null \
  && ok "asustuner-backend 运行中（socket: /run/asustuner-backend.sock）" \
  || die "服务未启动：journalctl -u asustuner-backend"

info "托盘桌面自启动"
cat > /usr/share/applications/asustuner-tray.desktop <<'DESK'
[Desktop Entry]
Type=Application
Name=AsusTuner Tray
Exec=/usr/bin/asustuner-tray
X-GNOME-Autostart-enabled=true
X-KDE-autostart-after=panel
Terminal=false
Categories=System;
DESK
mkdir -p /etc/xdg/autostart
install -m 644 /usr/share/applications/asustuner-tray.desktop /etc/xdg/autostart/asustuner-tray.desktop
ok "托盘将在登录后自启（也可手动运行 asustuner-tray）"

echo
ok "安装完成。"
echo "  托盘：登录后自动出现（右键菜单控制）"
echo "  主界面：托盘菜单 → 打开主界面（按需启动，关闭即释放内存）"
echo "  状态：systemctl status asustuner-backend / journalctl -u asustuner-backend -f"
echo "  卸载：systemctl disable --now asustuner-backend && rm /usr/bin/asustuner-*"

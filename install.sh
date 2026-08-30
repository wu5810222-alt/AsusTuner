#!/usr/bin/env bash
# AsusTuner 部署（方案 B 三件套）—— sudo 执行一次
#
#   1. 编译 release
#   2. 安装 backend / tray / gui / cli 到 /usr/bin
#   3. 启用 asustuner-backend.service（root 常驻：状态持久化 + 睡眠/档位事件自愈）
#   4. 托盘加入桌面自启动
#   5. 安装应用图标（hicolor）与 GUI 桌面入口
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
# sudo 构建会把 target 文件变 root 属主，导致用户侧后续构建失败——自动归还
if [[ -n "${SUDO_USER:-}" ]]; then
  chown -R "$SUDO_USER" target 2>/dev/null || true
fi
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
# enable --now 对已在运行的服务不会重启（会继续跑旧二进制），必须显式 restart
systemctl enable asustuner-backend.service
systemctl restart asustuner-backend.service
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
Icon=asustuner
X-GNOME-Autostart-enabled=true
X-KDE-autostart-after=panel
Terminal=false
Categories=System;
DESK
mkdir -p /etc/xdg/autostart
install -m 644 /usr/share/applications/asustuner-tray.desktop /etc/xdg/autostart/asustuner-tray.desktop
# user 级单元备用：niri/hyprland 等无 XDG autostart 实现的合成器环境，
# 用户手动 `systemctl --user enable --now asustuner-tray`（不默认启用，避免与 autostart 双开）
install -d /usr/lib/systemd/user
install -m 644 systemd/asustuner-tray.service /usr/lib/systemd/user/asustuner-tray.service
ok "托盘将在登录后自启（也可手动运行 asustuner-tray）"

info "应用图标（hicolor）与 GUI 桌面入口"
for s in 512 256 128 64 48 32 24 22 16; do
  install -d "/usr/share/icons/hicolor/${s}x${s}/apps"
  install -m 644 "assets/icons/hicolor/${s}x${s}/apps/asustuner.png" \
                 "/usr/share/icons/hicolor/${s}x${s}/apps/asustuner.png"
done
# 缓存刷新失败不阻塞安装（KDE 不强依赖，GNOME 等按 mtime 也能查到新图标）
gtk-update-icon-cache -qf /usr/share/icons/hicolor 2>/dev/null || true
cat > /usr/share/applications/asustuner.desktop <<'DESK'
[Desktop Entry]
Type=Application
Name=AsusTuner
GenericName=ASUS Laptop Control Center
Comment=风扇曲线 / 性能档位 / 功耗与 GPU 模式（asusd 前端）
Exec=/usr/bin/asustuner-gui
Icon=asustuner
Terminal=false
Categories=System;Settings;HardwareSettings;
Keywords=asus;fan;power;performance;rog;tuf;
DESK
ok "启动器已就绪（应用菜单搜 AsusTuner）"

echo
info "兼容性检查（仅提示，不影响安装）"
if command -v gnome-shell >/dev/null 2>&1; then
  info "  GNOME 桌面：托盘(SNI)需扩展支持，请确认已装 gnome-shell-extension-appindicator"
fi
if ! systemctl list-unit-files 2>/dev/null | grep -q '^asusd\.service'; then
  info "  ⚠ 未检测到 asusd：档位/充电/风扇曲线/灯效不可用（非华硕机器或未装 asusctl）"
fi
if [[ ! -x /usr/sbin/ryzenadj ]]; then
  info "  ⚠ 未安装 ryzenadj：AMD 降压/温度墙不可用（Intel 机型属正常）"
fi

echo
ok "安装完成。"
echo "  托盘：登录后自动出现（右键菜单控制）"
echo "  主界面：托盘菜单 → 打开主界面（按需启动，关闭即释放内存）"
echo "  状态：systemctl status asustuner-backend / journalctl -u asustuner-backend -f"
echo "  无 XDG autostart 的桌面（niri/hyprland 等）：systemctl --user enable --now asustuner-tray"
echo "  卸载：systemctl disable --now asustuner-backend && rm /usr/bin/asustuner-*"

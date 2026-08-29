#!/usr/bin/env bash
# AsusTuner 启动脚本（精简版）
#
# 架构：GUI/CLI 直连系统已有的 asusd (xyz.ljones.Asusd) + ryzenadj，
#       无需自研守护进程，无需 root（asusd 本身以 root 运行）。
#
# 用法：
#   ./run.sh            # 启动 GUI（KDE 窗口，直连 asusd）
#   ./run.sh --cli      # 显示 CLI 命令提示
#   ./run.sh --help     # 帮助

set -euo pipefail
cd "$(dirname "$0")"

RELEASE=target/release
GUIPATH="$RELEASE/asustuner-gui"
CLIPATH="$RELEASE/asustuner-cli"

BOLD=$'\033[1m'; CYAN=$'\033[36m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RED=$'\033[31m'; RESET=$'\033[0m'
info(){ echo "${CYAN}[AsusTuner]${RESET} $*"; }
ok(){ echo "${GREEN}[AsusTuner]${RESET} $*"; }
die(){ echo "${RED}[AsusTuner]${RESET} $*" >&2; exit 1; }

WANT_CLI=0
for arg in "$@"; do
  case "$arg" in
    --cli) WANT_CLI=1 ;;
    -h|--help)
      echo "用法: ./run.sh [--cli]"
      echo "  直接启动 GUI；--cli 显示 CLI 提示"
      exit 0 ;;
    *) die "未知参数: $arg" ;;
  esac
done

# 编译（若未构建）
if [[ ! -x "$GUIPATH" ]]; then
  info "构建 release..."
  cargo build --release || die "编译失败"
  ok "编译完成"
else
  ok "使用已编译二进制"
fi

# 确认 asusd 在跑（备用提示）
if ! pgrep -x asusd >/dev/null 2>&1; then
  warn "未检测到 asusd 服务。请先：systemctl enable --now asusd"
fi

echo
info "CLI 命令示例："
echo "  $CLIPATH profile               # 当前性能档位"
echo "  $CLIPATH set-profile balanced  # 设置性能档位"
echo "  $CLIPATH sensors               # 实时监控"
echo "  $CLIPATH power 45000 65000 45000  # 功率墙(需 ryzenadj 可访问硬件)"
echo

if [[ "$WANT_CLI" -eq 1 ]]; then
  info "CLI 模式。--help 查看全部命令："
  "$CLIPATH" --help 2>/dev/null | sed 's/^/  /' | head -25
else
  info "启动 GUI（关闭窗口退出）..."
  "$GUIPATH"
  info "GUI 已退出"
fi

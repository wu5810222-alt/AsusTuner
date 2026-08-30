#!/usr/bin/env bash
# 从透明主图生成应用图标全套尺寸（hicolor 布局，托盘内嵌 PNG 同源）
#
#   用法: scripts/make_icons.sh [master.png]
#   默认主图: assets/icon.png（1024x1024 RGBA，见 commit "feat: app icon"）
#
# 产物:
#   assets/icons/hicolor/<N>x<N>/apps/asustuner.png   — install.sh 安装目标布局
#   托盘 (asustuner-tray/src/main.rs) 通过 include_bytes! 直接引用
#   24/32/48 三档，改尺寸集后需重编译托盘。
set -euo pipefail
cd "$(dirname "$0")/.."

MASTER="${1:-assets/icon.png}"
[[ -f "$MASTER" ]] || { echo "主图不存在: $MASTER" >&2; exit 1; }
command -v magick >/dev/null || { echo "需要 ImageMagick 7 (magick)" >&2; exit 1; }

SIZES=(512 256 128 64 48 32 24 22 16)
for s in "${SIZES[@]}"; do
  out="assets/icons/hicolor/${s}x${s}/apps/asustuner.png"
  mkdir -p "$(dirname "$out")"
  magick "$MASTER" -resize "${s}x${s}" -strip "$out"
done

echo "已生成: ${SIZES[*]} -> assets/icons/hicolor/"

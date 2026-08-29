#!/usr/bin/env bash
# asus-armoury 功率墙写入实验 —— 验证三件事（结果贴回主对话裁决）：
#   T1 写入通道是否可用（先写同值）
#   T2 写异值是否生效 + 与 ryzenadj -i 交叉对读
#   T3 切档位后 ppt 值是否被 BIOS/驱动按档位重写（与 asusd EnablePptGroup 联动？）
# 安全性：只做 ±5W 可逆改动；写前记录原值，结束/出错均恢复。
#
# 用法：sudo ./scripts/armoury-experiment.sh
# 之后按脚本末尾指引做 D-Bus 免根写入试探与重启持久化检查。

set -u
CYAN=$'\033[36m'; GREEN=$'\033[32m'; RED=$'\033[31m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'
step(){ echo; echo "${CYAN}── $* ──${RESET}"; }
pass(){ echo "${GREEN}✓ $*${RESET}"; }
fail(){ echo "${RED}✗ $*${RESET}"; }
warn(){ echo "${YELLOW}! $*${RESET}"; }

BASE=/sys/class/firmware-attributes/asus-armoury/attributes
PL1=$BASE/ppt_pl1_spl    # STAPM 持续
PL2=$BASE/ppt_pl2_sppt   # SLOW 平均
PL3=$BASE/ppt_pl3_fppt   # FAST 瞬时

[[ "$(id -u)" -eq 0 ]] || { echo "请用 sudo 运行"; exit 1; }
for d in "$PL1" "$PL2" "$PL3"; do
  [[ -d "$d" ]] || { echo "未找到 $d —— 本机无 asus-armoury 接口？"; exit 1; }
done

orig1=$(cat "$PL1/current_value")
orig2=$(cat "$PL2/current_value")
orig3=$(cat "$PL3/current_value")
max1=$(cat "$PL1/max_value")
echo "原始值: PL1=${orig1}W PL2=${orig2}W PL3=${orig3}W (max ${max1}W)"

restore() {
  echo "$orig1" > "$PL1/current_value" 2>/dev/null
  echo "$orig2" > "$PL2/current_value" 2>/dev/null
  echo "$orig3" > "$PL3/current_value" 2>/dev/null
  echo "${GREEN}[恢复] PL1=$orig1 PL2=$orig2 PL3=$orig3${RESET}"
}
trap restore EXIT

# ---------- T1 写同值（通道验证） ----------
step "T1 写同值验证通道 (PL1 = ${orig1})"
echo "$orig1" > "$PL1/current_value" 2>/tmp/aw_err
rc=$?
if [[ $rc -ne 0 ]]; then
  fail "写入返回错误 rc=$rc: $(cat /tmp/aw_err)"
else
  now=$(cat "$PL1/current_value" 2>/dev/null)
  if [[ "$now" == "$orig1" ]]; then pass "写入成功且读回一致 ($now)"; else fail "读回 $now ≠ 写入 $orig1"; fi
fi

# ---------- T2 写异值（+5W）并交叉对读 ----------
step "T2 PL1 写异值 (+5W) 并读回"
target=$(( orig1 + 5 ))
[[ $target -le $max1 ]] || target=$(( orig1 - 5 ))
echo "目标 PL1 = ${target}W"
echo "$target" > "$PL1/current_value" 2>/dev/null
sleep 0.3
now=$(cat "$PL1/current_value")
if [[ "$now" == "$target" ]]; then
  pass "固件已接受并保持 ($now W)"
else
  fail "写入后读回 $now ≠ $target（固件可能拒绝/回弹）"
fi

step "ryzenadj -i 交叉对读（关注 STAPM/PPT 行，请人工比对量级）"
ryzenadj -i 2>/dev/null | grep -iE "stapm|ppt limit|fast|slow" || warn "ryzenadj -i 无输出"

# ---------- T3 档位联动（切档位后 ppt 是否被重写） ----------
step "T3 档位联动：切换档位后观察 ppt 是否被重写"
cur=$(gdbus call --system --dest xyz.ljones.Asusd --object-path /xyz/ljones \
  --method org.freedesktop.DBus.Properties.Get xyz.ljones.Platform PlatformProfile 2>/dev/null | \
  grep -oE "uint32 [0-9]+" | grep -oE "[0-9]+")
cur=${cur:-0}
if [[ "$cur" == "2" ]]; then other=0; oname=balanced; cname=quiet; else other=2; oname=quiet; cname=balanced; fi
echo "当前档位 $cname($cur)，切到 $oname($other)..."
gdbus call --system --dest xyz.ljones.Asusd --object-path /xyz/ljones \
  --method org.freedesktop.DBus.Properties.Set xyz.ljones.Platform PlatformProfile "<uint32 $other>" >/dev/null 2>&1
sleep 1.5
p1=$(cat "$PL1/current_value"); p2=$(cat "$PL2/current_value"); p3=$(cat "$PL3/current_value")
echo "切换后: PL1=${p1}W PL2=${p2}W PL3=${p3}W"
if [[ "$p1" == "$target" && "$p2" == "$orig2" && "$p3" == "$orig3" ]]; then
  pass "ppt 值未随档位变化（独立于档位）"
else
  warn "ppt 值随档位被重写了！→ 功率墙与档位联动，切档即重置（重要发现）"
fi

echo "切回 $cname($cur)..."
gdbus call --system --dest xyz.ljones.Asusd --object-path /xyz/ljones \
  --method org.freedesktop.DBus.Properties.Set xyz.ljones.Platform PlatformProfile "<uint32 $cur>" >/dev/null 2>&1
sleep 1

# ---------- 结束 ----------
step "实验结束（已自动恢复原值）"
echo "请把以上全部输出贴回主对话。另外两项请手动完成："
echo
echo "${YELLOW}A) D-Bus 免根写入试探（在普通用户终端跑，非 sudo）:${RESET}"
echo "   gdbus call --system --dest xyz.ljones.Asusd --object-path /xyz/ljones/asus_armoury/ppt_pl1_spl \\"
echo "     --method org.freedesktop.DBus.Properties.Set xyz.ljones.AsusArmoury CurrentValue '<int32 ${orig1}>'"
echo "   成功(无报错)→功率墙可免 root 写 asusd；报错→需走 root 后端写 sysfs"
echo
echo "${YELLOW}B) 重启持久化检查:${RESET}"
echo "   1. 手动写入一个不同值:  sudo bash -c 'echo $(( orig1 + 3 )) > $PL1/current_value'"
echo "   2. 重启"
echo "   3. cat $PL1/current_value —— 若仍是 $(( orig1 + 3 )) 则固件持久化（功率墙可移出重放名单）；回到 $orig1 则不持久"

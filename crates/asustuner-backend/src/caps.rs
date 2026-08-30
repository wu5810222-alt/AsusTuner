//! 平台能力探测（兼容层）：
//! 运行时探测 ASUS 固件接口 / AMD 功控 / 温度源 / RAPL 域是否可用。
//! 后端据此跳过不可用项（而非堆积失败明细），GUI 可经 get_state 读取裁剪 UI。
//!
//! 所有文件系统探测都走 `root` 前缀（真实机器传 `/`，单测可注入临时目录
//! 伪造 sysfs 树）；asusd 是 D-Bus 探测，无法注入，单独函数。
//!
//! 该模块是「预留平台抽象层」的接缝：将来支持非 ASUS 机器时，把各
//! asusd/armoury 调用点改为按 Caps 分发实现即可。

use std::path::Path;
use std::sync::LazyLock;

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Caps {
    /// asusd 服务可达（档位/充电/风扇曲线）——启动时探测一次，信息性字段；
    /// 实际调用仍按原有重试逻辑，不据此跳过
    pub asusd: bool,
    /// asus-armoury 功率墙固件属性存在（ppt_pl1_spl 可写）
    pub armoury_ppt: bool,
    /// hwmon name=asus 且含 fan1_input（双风扇 RPM 只读）
    pub asus_hwmon: bool,
    /// asus::kbd_backlight LED（亮度/Aura）
    pub kbd_led: bool,
    /// AMD 功控可用：ryzenadj 存在 **且** CPU vendor 为 AuthenticAMD（与）
    /// 覆盖：降压 coall/cogfx、温度墙 tctl、功率墙 ryzenadj 回退
    pub amd_adj: bool,
    /// CPU RAPL 能量域目录名（瞬时功率差分），如 "intel-rapl:0" / "amd-rapl:0"
    pub rapl_dir: Option<String>,
    /// CPU 温度源 hwmon name（k10temp / coretemp）
    pub cpu_temp_src: Option<String>,
    /// iGPU 温度源 hwmon name（amdgpu / i915）
    pub igpu_temp_src: Option<String>,
}

static CAPS: LazyLock<Caps> = LazyLock::new(detect);

/// 全局能力快照（进程内只探测一次）。
pub fn get() -> &'static Caps {
    &CAPS
}

/// 真实机器探测（文件系统 + asusd D-Bus）。
pub fn detect() -> Caps {
    let mut c = detect_with_root(Path::new("/"));
    c.asusd = probe_asusd();
    c
}

/// 仅文件系统探测（root 可注入，供单测）。asusd 字段恒为 false，调用方按需补。
pub fn detect_with_root(root: &Path) -> Caps {
    Caps {
        asusd: false,
        armoury_ppt: root
            .join("sys/class/firmware-attributes/asus-armoury/attributes/ppt_pl1_spl/current_value")
            .exists(),
        asus_hwmon: find_asus_fan_hwmon(root).is_some(),
        kbd_led: root
            .join("sys/class/leds/asus::kbd_backlight/brightness")
            .exists(),
        amd_adj: cpu_vendor_is_amd(root) && root.join("usr/sbin/ryzenadj").exists(),
        rapl_dir: find_rapl_dir(root),
        cpu_temp_src: find_hwmon(root, &["k10temp", "coretemp"]),
        igpu_temp_src: find_hwmon(root, &["amdgpu", "i915"]),
    }
}

/// asusd 是否可达（一次属性读，毫秒级）。
pub fn probe_asusd() -> bool {
    (|| -> anyhow::Result<()> {
        let conn = zbus::blocking::Connection::system()?;
        let p = zbus::blocking::Proxy::new_owned(
            conn,
            "xyz.ljones.Asusd",
            "/xyz/ljones",
            "xyz.ljones.Platform",
        )?;
        let _: u32 = p.get_property("PlatformProfile")?;
        Ok(())
    })()
    .is_ok()
}

fn cpu_vendor_is_amd(root: &Path) -> bool {
    std::fs::read_to_string(root.join("proc/cpuinfo"))
        .map(|s| {
            s.lines().any(|l| {
                let l = l.trim();
                l.starts_with("vendor_id") && l.contains("AuthenticAMD")
            })
        })
        .unwrap_or(false)
}

/// 在 hwmon 目录中按候选顺序找第一个 name 命中的（候选互斥，命中序即优先级）。
fn find_hwmon(root: &Path, candidates: &[&str]) -> Option<String> {
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("sys/class/hwmon"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort(); // 探测顺序稳定（ hwmon 实例号非稳定命名）
    for d in dirs {
        let name = std::fs::read_to_string(d.join("name")).unwrap_or_default();
        let name = name.trim();
        if let Some(c) = candidates.iter().find(|c| **c == name) {
            return Some((*c).to_string());
        }
    }
    None
}

fn find_asus_fan_hwmon(root: &Path) -> Option<String> {
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("sys/class/hwmon"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for d in dirs {
        let is_asus = std::fs::read_to_string(d.join("name"))
            .map(|n| n.trim() == "asus")
            .unwrap_or(false);
        if is_asus && d.join("fan1_input").exists() {
            return d.file_name().map(|n| n.to_string_lossy().into_owned());
        }
    }
    None
}

/// RAPL CPU 能量域：优先 name=="cpu" 的域（Intel/AMD 均用此命名 package 域），
/// 兜底 intel-rapl:0（老内核无 name 文件）→ 再兜底第一个含 energy_uj 的目录。
/// 排序保证 intel-rapl:0 先于 intel-rapl:1 等兄弟域。
fn find_rapl_dir(root: &Path) -> Option<String> {
    let base = root.join("sys/class/powercap");
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(&base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    let mut fallback = None;
    for d in dirs {
        if !d.join("energy_uj").exists() {
            continue;
        }
        let fname = d.file_name()?.to_string_lossy().into_owned();
        let name = std::fs::read_to_string(d.join("name")).unwrap_or_default();
        if name.trim() == "cpu" {
            return Some(fname);
        }
        if fallback.is_none() {
            fallback = Some(fname);
        }
    }
    if base.join("intel-rapl:0/energy_uj").exists() {
        return Some("intel-rapl:0".into());
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn put(base: &Path, path: &str, content: &str) {
        let full = base.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("asut-caps-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rapl_prefers_cpu_named_domain() {
        let t = tmpdir("rapl-amd");
        put(&t, "sys/class/powercap/amd-rapl:0/name", "cpu\n");
        put(&t, "sys/class/powercap/amd-rapl:0/energy_uj", "1");
        put(&t, "sys/class/powercap/amd-rapl:0:0/name", "core\n");
        put(&t, "sys/class/powercap/intel-rapl:0/name", "uncore\n");
        put(&t, "sys/class/powercap/intel-rapl:0/energy_uj", "1");
        let caps = detect_with_root(&t);
        assert_eq!(caps.rapl_dir.as_deref(), Some("amd-rapl:0"));
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn rapl_falls_back_to_intel_rapl0_without_name() {
        let t = tmpdir("rapl-legacy");
        fs::create_dir_all(t.join("sys/class/powercap/intel-rapl:0")).unwrap();
        put(&t, "sys/class/powercap/intel-rapl:0/energy_uj", "1");
        let caps = detect_with_root(&t);
        assert_eq!(caps.rapl_dir.as_deref(), Some("intel-rapl:0"));
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn temp_sources_probe_in_order() {
        let t = tmpdir("temps");
        put(&t, "sys/class/hwmon/hwmon0/name", "coretemp\n");
        put(&t, "sys/class/hwmon/hwmon1/name", "i915\n");
        put(&t, "sys/class/hwmon/hwmon2/name", "acpitz\n");
        let caps = detect_with_root(&t);
        assert_eq!(caps.cpu_temp_src.as_deref(), Some("coretemp"));
        assert_eq!(caps.igpu_temp_src.as_deref(), Some("i915"));
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn amd_adj_requires_vendor_and_binary() {
        let t = tmpdir("amd");
        // 只有二进制，无 AMD vendor
        put(&t, "usr/sbin/ryzenadj", "#!/bin/sh\n");
        put(&t, "proc/cpuinfo", "vendor_id : GenuineIntel\n");
        assert!(!detect_with_root(&t).amd_adj);
        // vendor 命中
        put(&t, "proc/cpuinfo", "vendor_id : AuthenticAMD\n");
        assert!(detect_with_root(&t).amd_adj);
        // 只有 vendor，无二进制
        fs::remove_file(t.join("usr/sbin/ryzenadj")).unwrap();
        assert!(!detect_with_root(&t).amd_adj);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn asus_specific_caps_detected() {
        let t = tmpdir("asus");
        put(&t, "sys/class/hwmon/hwmon9/name", "asus\n");
        put(&t, "sys/class/hwmon/hwmon9/fan1_input", "3000\n");
        put(&t, "sys/class/leds/asus::kbd_backlight/brightness", "0\n");
        put(
            &t,
            "sys/class/firmware-attributes/asus-armoury/attributes/ppt_pl1_spl/current_value",
            "100",
        );
        let caps = detect_with_root(&t);
        assert!(caps.asus_hwmon);
        assert!(caps.kbd_led);
        assert!(caps.armoury_ppt);
        assert!(!caps.asusd); // D-Bus 不在文件探测范围内
        fs::remove_dir_all(&t).ok();
    }
}

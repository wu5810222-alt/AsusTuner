//! AsusTuner 命令行客户端 —— 直连 asusd (xyz.ljones.Asusd) + ryzenadj。
//!
//! 用法示例：
//!   asustuner-cli sensors          # 实时监控（只读 sysfs）
//!   asustuner-cli profile          # 当前性能档位
//!   asustuner-cli profile balanced # 设置性能档位（quiet/balanced/performance/lowpower）
//!   asustuner-cli charge 80        # 设置充电限制
//!   asustuner-cli power 45000 65000 45000  # 功率墙 mW
//!   asustuner-cli curve -30 -20    # CPU 降压（负值=降压）
//!   asustuner-cli fan-get 1        # 读取风扇曲线

pub mod fan;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

const ASUSD_SERVICE: &str = "xyz.ljones.Asusd";
const ASUSD_PATH: &str = "/xyz/ljones";
const PLATFORM_IFACE: &str = "xyz.ljones.Platform";
const RYZENADJ: &str = "/usr/sbin/ryzenadj";

#[derive(Parser)]
#[command(name = "asustuner-cli", about = "AsusTuner 命令行客户端（连 asusd + ryzenadj）", version)]
struct Cli {
    /// 用 RyzenAdj 的只读信息模式连 asusd（默认自动）
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 实时监控（只读 sysfs，无需 root）
    Sensors,
    /// 查看当前性能档位
    Profile,
    /// 设置性能档位（quiet/balanced/performance/lowpower）
    SetProfile { profile: String },
    /// 设置充电限制 (%)
    Charge { limit: u32 },
    /// 设置功率墙（mW）：STAPM / FAST / SLOW
    Power { stapm: u32, fast: u32, slow: u32 },
    /// 设置 CPU 降压（负值=降压，如 -30）
    Curve {
        #[arg(allow_hyphen_values = true)]
        all_cores: i32,
        #[arg(allow_hyphen_values = true)]
        igpu: i32,
    },
    /// 读取风扇曲线（asusd FanCurves）
    FanGet { fan: u32 },
    /// 设置风扇曲线：--temp "56,61,66,71,76,80,85,97" --pwm "0,10,30,60,80,120,180,229"
    /// （CPU 风扇；可选 --gpu 设为 GPU 风扇）
    FanSet {
        #[arg(long)]
        temp: String,
        #[arg(long)]
        pwm: String,
        #[arg(long)]
        gpu: bool,
    },
}

/// 连接 asusd 并返回 Platform 代理。
fn asusd() -> Result<zbus::blocking::Proxy<'static>> {
    let conn = zbus::blocking::Connection::system().context("连接 system bus 失败")?;
    zbus::blocking::Proxy::new_owned(conn, ASUSD_SERVICE, ASUSD_PATH, PLATFORM_IFACE)
        .context("创建 asusd 代理失败")
}

/// 平台档位数值 <-> 名称。
fn profile_name(v: u32) -> &'static str {
    match v {
        0 => "balanced",
        1 => "performance",
        2 => "quiet",
        3 => "lowpower",
        _ => "unknown",
    }
}
fn profile_val(s: &str) -> Option<u32> {
    match s.to_ascii_lowercase().as_str() {
        "balanced" => Some(0),
        "performance" | "turbo" => Some(1),
        "quiet" | "silent" => Some(2),
        "lowpower" => Some(3),
        _ => None,
    }
}

fn run_ryzenadj(args: &[String]) -> Result<()> {
    let out = std::process::Command::new(RYZENADJ)
        .args(args)
        .output()
        .context("执行 ryzenadj 失败")?;
    if !out.status.success() {
        anyhow::bail!("ryzenadj 错误: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Sensors => print_sensors(),
        Cmd::Profile => {
            let p = asusd()?;
            let v: u32 = p.get_property("PlatformProfile")?;
            let choice: Vec<u32> = p.get_property("PlatformProfileChoices")?;
            println!("当前性能档位: {} ({v})", profile_name(v));
            println!("可用: {:?}", choice.iter().map(|&c| profile_name(c)).collect::<Vec<_>>());
        }
        Cmd::SetProfile { profile } => {
            let val = profile_val(&profile).context("未知档位（可选 quiet/balanced/performance/lowpower）")?;
            let p = asusd()?;
            p.set_property::<u32>("PlatformProfile", val)?;
            println!("性能档位已设为: {}", profile_name(val));
        }
        Cmd::Charge { limit } => {
            let p = asusd()?;
            p.set_property::<u8>("ChargeControlEndThreshold", limit as u8)?;
            println!("充电限制已设为 {limit}%");
        }
        Cmd::Power { stapm, fast, slow } => {
            let args = vec![
                format!("--stapm-limit={stapm}"),
                format!("--fast-limit={fast}"),
                format!("--slow-limit={slow}"),
            ];
            run_ryzenadj(&args)?;
            println!("功率墙: STAPM={stapm}mW, FAST={fast}mW, SLOW={slow}mW");
        }
        Cmd::Curve { all_cores, igpu } => {
            let mut args = Vec::new();
            if all_cores != 0 {
                args.push(format!("--set-coall={all_cores}"));
            }
            if igpu != 0 {
                args.push(format!("--set-cogfx={igpu}"));
            }
            run_ryzenadj(&args)?;
            println!("CPU 降压: all_cores={all_cores}, igpu={igpu}");
        }
        Cmd::FanGet { fan: _ } => {
            let conn = zbus::blocking::Connection::system()?;
            let p = zbus::blocking::Proxy::new_owned(
                conn,
                "xyz.ljones.Asusd",
                "/xyz/ljones",
                "xyz.ljones.Platform",
            )?;
            let profile: u32 = p.get_property("PlatformProfile")?;
            let curves = fan::read_fan_curves(profile)?;
            println!("当前档位 {} 的风扇曲线：", profile_name(profile));
            for c in &curves {
                let fan = c.fan.as_str();
                let enabled = if c.enabled { "启用" } else { "关闭" };
                println!("  风扇 {fan}（{enabled}）:");
                for i in 0..8 {
                    println!("    {}°C -> {}%", c.temp[i], (c.pwm[i] as u32) * 100 / 255);
                }
            }
        }
        Cmd::FanSet { temp, pwm, gpu } => {
            let temps = parse_u8_array(&temp)?;
            let pwms = parse_u8_array(&pwm)?;
            if temps.len() != 8 || pwms.len() != 8 {
                anyhow::bail!("温度与 PWM 各需 8 个点，当前 {} 和 {}", temps.len(), pwms.len());
            }
            let mut t = [0u8; 8];
            let mut p = [0u8; 8];
            t.copy_from_slice(&temps);
            p.copy_from_slice(&pwms);
            let profile = current_profile()?;
            let curve = fan::CurveData {
                fan: if gpu { fan::FanCurvePU::GPU } else { fan::FanCurvePU::CPU },
                pwm: p,
                temp: t,
                enabled: true,
            };
            fan::write_fan_curve(profile, curve)?;
            println!("已设置 {} 风扇曲线（档位 {}）", if gpu { "GPU" } else { "CPU" }, profile_name(profile));
        }
    }
    Ok(())
}

/// 读当前性能档位值（asusd Platform，u32）。
fn current_profile() -> anyhow::Result<u32> {
    let conn = zbus::blocking::Connection::system()?;
    let p = zbus::blocking::Proxy::new_owned(
        conn,
        "xyz.ljones.Asusd",
        "/xyz/ljones",
        "xyz.ljones.Platform",
    )?;
    Ok(p.get_property("PlatformProfile")?)
}

/// 解析逗号分隔的 u8 数组，如 "56,61,66"。
fn parse_u8_array(s: &str) -> Result<Vec<u8>> {
    s.split(',')
        .filter(|x| !x.trim().is_empty())
        .map(|x| x.trim().parse::<u8>().map_err(|e| anyhow::anyhow!("数值无效 '{}': {e}", x)))
        .collect()
}

fn print_sensors() {
    println!("=== 实时监控（只读 sysfs）===");
    if let Some(t) = read_temp("k10temp") {
        println!("CPU 温度: {t:.1} °C");
    }
    if let Some(t) = read_temp("amdgpu") {
        println!("iGPU 温度: {t:.1} °C");
    }
    if let Some(f) = read_cpu_freq() {
        println!("CPU 频率: {f:.0} MHz");
    }
    if let Some(b) = read_battery() {
        println!("电池: {b}");
    }
}

fn read_temp(name: &str) -> Option<f64> {
    let dir = std::fs::read_dir("/sys/class/hwmon").ok()?;
    for e in dir.flatten() {
        let d = e.path();
        let n = std::fs::read_to_string(d.join("name")).ok()?;
        if n.trim() == name {
            for i in 1..=2 {
                if let Some(v) = read_f64(&d.join(format!("temp{i}_input"))) {
                    return Some(v / 1000.0);
                }
            }
        }
    }
    None
}

fn read_cpu_freq() -> Option<f64> {
    let dir = std::fs::read_dir("/sys/devices/system/cpu").ok()?;
    let mut sum = 0.0;
    let mut n = 0.0;
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
            if let Some(v) = read_f64(&e.path().join("cpufreq/scaling_cur_freq")) {
                sum += v / 1000.0;
                n += 1.0;
            }
        }
    }
    if n > 0.0 { Some(sum / n) } else { None }
}

fn read_battery() -> Option<String> {
    let cap = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity").ok()?.trim().parse::<f64>().ok()?;
    let status = std::fs::read_to_string("/sys/class/power_supply/BAT0/status").ok()?.trim().to_string();
    Some(format!("{cap:.0}% ({status})"))
}

fn read_f64(p: &std::path::Path) -> Option<f64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

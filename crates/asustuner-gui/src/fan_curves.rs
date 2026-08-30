//! asusd `FanCurves` D-Bus 接口的本地类型映射。
//!
//! 复刻 asusd 的 `CurveData` / `FanCurvePU`（见 asusctl 的 rog-profiles crate），
//! 用 zbus v4 的 `#[proxy]` 宏强类型调用，避免手拼 D-Bus 消息。

use serde::{Deserialize, Serialize};
use zbus::proxy;
use zvariant::Type;

/// 风扇编号（D-Bus 上为字符串签名 "s"）。
/// 对应 asusd `FanCurvePU`：CPU=0 / GPU=1 / MID=2。
#[derive(Type, Deserialize, Serialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[zvariant(signature = "s")]
#[repr(u8)]
pub enum FanCurvePU {
    #[default]
    CPU = 0,
    GPU = 1,
    MID = 2,
}

impl FanCurvePU {
    pub fn as_str(&self) -> &'static str {
        match self {
            FanCurvePU::CPU => "CPU",
            FanCurvePU::GPU => "GPU",
            FanCurvePU::MID => "MID",
        }
    }
}

/// 一条风扇曲线数据（对应 asusd `CurveData`）。
/// 8 个温度点 + 8 个 PWM 档 + 是否启用。
#[derive(Type, Deserialize, Serialize, Default, Debug, Clone)]
pub struct CurveData {
    pub fan: FanCurvePU,
    pub pwm: [u8; 8],
    pub temp: [u8; 8],
    pub enabled: bool,
}

/// `PlatformProfile` 在 asusd 的 D-Bus 上为无符号整数签名 "u"。
pub type PlatformProfile = u32;

/// asusd `FanCurves` 接口 proxy。
#[proxy(
    interface = "xyz.ljones.FanCurves",
    default_service = "xyz.ljones.Asusd",
    default_path = "/xyz/ljones"
)]
pub trait FanCurves {
    /// 当前平台的 fan-curve 数据（全部风扇）。
    fn fan_curve_data(&self, profile: PlatformProfile) -> zbus::Result<Vec<CurveData>>;

    /// 恢复为默认曲线。
    fn set_curves_to_defaults(&self, profile: PlatformProfile) -> zbus::Result<()>;

    /// 设置一条风扇曲线（会自动激活）。profile 为档位。
    fn set_fan_curve(&self, profile: PlatformProfile, curve: CurveData) -> zbus::Result<()>;

    /// 设置某档位全风扇曲线启用状态。
    fn set_fan_curves_enabled(&self, profile: PlatformProfile, enabled: bool) -> zbus::Result<()>;
}

use std::sync::{LazyLock, Mutex};

/// 全局 FanCurves 代理（缓存，避免每次重建连接）。
static FC_PROXY: LazyLock<Mutex<Option<zbus::blocking::Connection>>> =
    LazyLock::new(|| Mutex::new(None));

fn conn() -> anyhow::Result<zbus::blocking::Connection> {
    let mut g = FC_PROXY.lock().unwrap();
    if g.is_none() {
        *g = zbus::blocking::Connection::system().ok();
    }
    g.as_ref()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("D-Bus system bus 连接失败（asusd 未运行？）"))
}

/// 读取当前档位的全部风扇曲线。
pub fn read_fan_curves(profile: u32) -> anyhow::Result<Vec<CurveData>> {
    let proxy = FanCurvesProxyBlocking::new(&conn()?)?;
    let curves = proxy.fan_curve_data(profile)?;
    Ok(curves)
}

/// 写入一条风扇曲线到指定档位（会自动激活）。
pub fn write_fan_curve(profile: u32, curve: CurveData) -> anyhow::Result<()> {
    let proxy = FanCurvesProxyBlocking::new(&conn()?)?;
    proxy.set_fan_curve(profile, curve)?;
    Ok(())
}

/// 恢复当前档位风扇曲线为默认。
pub fn restore_defaults(profile: u32) -> anyhow::Result<()> {
    let proxy = FanCurvesProxyBlocking::new(&conn()?)?;
    proxy.set_curves_to_defaults(profile)?;
    Ok(())
}

// 简单测试 CurveData 默认值语义
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_default() {
        let c = CurveData::default();
        assert_eq!(c.fan, FanCurvePU::CPU);
        assert_eq!(c.pwm, [0; 8]);
        assert_eq!(c.temp, [0; 8]);
        assert!(!c.enabled);
    }

    #[test]
    fn fan_str() {
        assert_eq!(FanCurvePU::CPU.as_str(), "CPU");
        assert_eq!(FanCurvePU::GPU.as_str(), "GPU");
        assert_eq!(FanCurvePU::MID.as_str(), "MID");
    }
}

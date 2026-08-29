//! asusd `FanCurves` D-Bus 接口的本地类型映射（复刻 asusctl rog-profiles）。

use serde::{Deserialize, Serialize};
use zbus::proxy;
use zvariant::Type;

/// 风扇编号（D-Bus 字符串签名 "s"）。
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

/// 一条风扇曲线数据。
#[derive(Type, Deserialize, Serialize, Default, Debug, Clone)]
pub struct CurveData {
    pub fan: FanCurvePU,
    pub pwm: [u8; 8],
    pub temp: [u8; 8],
    pub enabled: bool,
}

/// asusd `FanCurves` 接口 proxy。
#[proxy(
    interface = "xyz.ljones.FanCurves",
    default_service = "xyz.ljones.Asusd",
    default_path = "/xyz/ljones"
)]
pub trait FanCurves {
    fn fan_curve_data(&self, profile: u32) -> zbus::Result<Vec<CurveData>>;
    fn set_curves_to_defaults(&self, profile: u32) -> zbus::Result<()>;
    fn set_fan_curve(&self, profile: u32, curve: CurveData) -> zbus::Result<()>;
}

/// 获取 FanCurves 代理并读数据。
pub fn read_fan_curves(profile: u32) -> anyhow::Result<Vec<CurveData>> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = FanCurvesProxyBlocking::new(&conn)?;
    let curves = proxy.fan_curve_data(profile)?;
    Ok(curves)
}

/// 写入一条风扇曲线到 asusd（profile 为档位，curve 为曲线数据，会自动激活）。
pub fn write_fan_curve(profile: u32, curve: CurveData) -> anyhow::Result<()> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = FanCurvesProxyBlocking::new(&conn)?;
    proxy.set_fan_curve(profile, curve)?;
    Ok(())
}

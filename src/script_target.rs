use anyhow::{Result, bail};

use crate::ids::{KeyId, LedId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunToTarget {
    Led(LedId),
    Key(KeyId),
    SegDigit(u8),
    Pin { port: usize, bit: u8 },
    I2cMasterScl,
    I2cMasterSda,
    I2cBusScl,
    I2cBusSda,
    I2cSlaveSclLow,
    I2cSlaveSdaLow,
    OnewireMasterHigh,
    OnewireBusHigh,
    OnewireDeviceLow,
    Uart1Tx,
    Uart1Rx,
    Uart2Tx,
    Uart2Rx,
    Ds1302Ce,
    Ds1302Clk,
    Ds1302Io,
    Ne555SigOut,
}

impl RunToTarget {
    pub fn parse(name: &str) -> Result<Self> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            bail!("run_to 目标不能为空");
        }

        if let Ok(led) = LedId::parse(trimmed) {
            return Ok(Self::Led(led));
        }
        if let Ok(key) = KeyId::parse(trimmed) {
            return Ok(Self::Key(key));
        }
        if let Some(target) = parse_seg_digit_target(trimmed) {
            return Ok(target);
        }
        if let Some(target) = parse_pin_target(trimmed) {
            return Ok(target);
        }

        let normalized = normalize_target_name(trimmed);
        match normalized.as_str() {
            "sigout" | "signalout" => Ok(Self::Pin { port: 3, bit: 4 }),
            "netsig" => Ok(Self::Ne555SigOut),
            "i2cscl" | "iicscl" => Ok(Self::I2cBusScl),
            "i2csda" | "iicsda" => Ok(Self::I2cBusSda),
            "i2cmasterscl" | "iicmasterscl" | "masterscl" => Ok(Self::I2cMasterScl),
            "i2cmastersda" | "iicmastersda" | "mastersda" => Ok(Self::I2cMasterSda),
            "i2cbusscl" | "iicbusscl" | "busscl" => Ok(Self::I2cBusScl),
            "i2cbussda" | "iicbussda" | "bussda" => Ok(Self::I2cBusSda),
            "i2cslavescllow" | "iicslavescllow" | "slavescllow" => Ok(Self::I2cSlaveSclLow),
            "i2cslavesdalow" | "iicslavesdalow" | "slavesdalow" => Ok(Self::I2cSlaveSdaLow),
            "onewiremaster" | "1wiremaster" | "1wiremasterhigh" => Ok(Self::OnewireMasterHigh),
            "onewirebus" | "1wirebus" | "1wirebushigh" => Ok(Self::OnewireBusHigh),
            "onewiredevice" | "1wiredevice" | "1wiredevicelow" => Ok(Self::OnewireDeviceLow),
            "onewiremasterhigh" | "wiremasterhigh" | "owmasterhigh" => Ok(Self::OnewireMasterHigh),
            "onewirebushigh" | "wirebushigh" | "owbushigh" => Ok(Self::OnewireBusHigh),
            "onewiredevicelow" | "wiredevicelow" | "owdevicelow" => Ok(Self::OnewireDeviceLow),
            "uart1tx" | "serial1tx" => Ok(Self::Uart1Tx),
            "uart1rx" | "serial1rx" => Ok(Self::Uart1Rx),
            "uart2tx" | "serial2tx" => Ok(Self::Uart2Tx),
            "uart2rx" | "serial2rx" => Ok(Self::Uart2Rx),
            "ds1302ce" | "rtcce" => Ok(Self::Ds1302Ce),
            "ds1302clk" | "rtcclk" => Ok(Self::Ds1302Clk),
            "ds1302io" | "rtcio" => Ok(Self::Ds1302Io),
            "ne555" | "ne555sigout" => Ok(Self::Ne555SigOut),
            _ => bail!("未知 run_to 目标: {trimmed}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunToEdge {
    Up,
    Down,
    Flip,
}

impl RunToEdge {
    pub fn parse(name: &str) -> Result<Self> {
        let normalized = normalize_target_name(name);
        match normalized.as_str() {
            "up" | "rise" | "rising" | "posedge" | "high" => Ok(Self::Up),
            "down" | "fall" | "falling" | "negedge" | "low" => Ok(Self::Down),
            "flip" | "toggle" | "change" => Ok(Self::Flip),
            _ => bail!("未知 run_to 边沿: {name}"),
        }
    }
}

fn normalize_target_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn parse_pin_target(name: &str) -> Option<RunToTarget> {
    let normalized = normalize_target_name(name);
    let digits = normalized.strip_prefix('p')?;
    if digits.len() != 2 && digits.len() != 3 {
        return None;
    }
    let (port_text, bit_text) = digits.split_at(digits.len() - 1);
    let port = port_text.parse::<usize>().ok()?;
    let bit = bit_text.parse::<u8>().ok()?;
    if port > 5 || bit > 7 {
        return None;
    }
    Some(RunToTarget::Pin { port, bit })
}

fn parse_seg_digit_target(name: &str) -> Option<RunToTarget> {
    let normalized = normalize_target_name(name);
    let digits = normalized
        .strip_prefix("seg")
        .or_else(|| normalized.strip_prefix('d'))?;
    let index = digits.parse::<u8>().ok()?;
    if !(1..=8).contains(&index) {
        return None;
    }
    Some(RunToTarget::SegDigit(index))
}

#[cfg(test)]
mod tests {
    use super::{RunToEdge, RunToTarget};
    use crate::ids::{KeyId, LedId};

    #[test]
    fn parse_run_to_target_supports_led_key_and_pin() {
        assert_eq!(
            RunToTarget::parse("L1").unwrap(),
            RunToTarget::Led(LedId::L1)
        );
        assert_eq!(
            RunToTarget::parse("S4").unwrap(),
            RunToTarget::Key(KeyId::S4)
        );
        assert_eq!(RunToTarget::parse("D8").unwrap(), RunToTarget::SegDigit(8));
        assert_eq!(
            RunToTarget::parse("P00").unwrap(),
            RunToTarget::Pin { port: 0, bit: 0 }
        );
        assert_eq!(
            RunToTarget::parse("P3.4").unwrap(),
            RunToTarget::Pin { port: 3, bit: 4 }
        );
    }

    #[test]
    fn parse_run_to_target_supports_protocol_aliases() {
        assert_eq!(
            RunToTarget::parse("iic_bus_scl").unwrap(),
            RunToTarget::I2cBusScl
        );
        assert_eq!(
            RunToTarget::parse("uart1_tx").unwrap(),
            RunToTarget::Uart1Tx
        );
        assert_eq!(
            RunToTarget::parse("1-wire bus").unwrap(),
            RunToTarget::OnewireBusHigh
        );
        assert_eq!(
            RunToTarget::parse("SIG_OUT").unwrap(),
            RunToTarget::Pin { port: 3, bit: 4 }
        );
        assert_eq!(
            RunToTarget::parse("NET_SIG").unwrap(),
            RunToTarget::Ne555SigOut
        );
    }

    #[test]
    fn parse_run_to_edge_supports_common_aliases() {
        assert_eq!(RunToEdge::parse("UP").unwrap(), RunToEdge::Up);
        assert_eq!(RunToEdge::parse("falling").unwrap(), RunToEdge::Down);
        assert_eq!(RunToEdge::parse("toggle").unwrap(), RunToEdge::Flip);
    }
}

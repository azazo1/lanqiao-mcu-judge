use std::fmt::Write as _;

use anyhow::{Result, bail};

const PREFIX: &str = "stcjudge_persistent_v1";
const MAX_SUB_NS_EXCLUSIVE: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersistentState {
    pub(crate) ds18b20: Ds18b20PersistentState,
    pub(crate) ds1302: Ds1302PersistentState,
    pub(crate) at24c02: At24c02PersistentState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ds18b20PersistentState {
    pub(crate) rom: [u8; 8],
    pub(crate) eeprom_th: u8,
    pub(crate) eeprom_tl: u8,
    pub(crate) eeprom_config: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ds1302PersistentState {
    pub(crate) write_protect: bool,
    pub(crate) trickle_charge: u8,
    pub(crate) ram: [u8; 31],
    pub(crate) hour_mode_12: bool,
    pub(crate) hour: u8,
    pub(crate) minute: u8,
    pub(crate) second: u8,
    pub(crate) day_of_week: u8,
    pub(crate) date: u8,
    pub(crate) month: u8,
    pub(crate) year: u8,
    pub(crate) halted: bool,
    pub(crate) sub_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct At24c02PersistentState {
    pub(crate) memory: [u8; 256],
}

impl PersistentState {
    pub(crate) fn encode(&self) -> String {
        let ds18b20_rom = hex_encode(&self.ds18b20.rom);
        let ds1302_ram = hex_encode(&self.ds1302.ram);
        let at24c02 = hex_encode(&self.at24c02.memory);

        let mut out = String::new();
        let _ = write!(
            out,
            "{PREFIX}|ds18b20={},{:02X},{:02X},{:02X}|ds1302={},{:02X},{},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{},{:016X}|ds1302_ram={}|at24c02={}",
            ds18b20_rom,
            self.ds18b20.eeprom_th,
            self.ds18b20.eeprom_tl,
            self.ds18b20.eeprom_config,
            u8::from(self.ds1302.write_protect),
            self.ds1302.trickle_charge,
            u8::from(self.ds1302.hour_mode_12),
            self.ds1302.hour,
            self.ds1302.minute,
            self.ds1302.second,
            self.ds1302.day_of_week,
            self.ds1302.date,
            self.ds1302.month,
            self.ds1302.year,
            u8::from(self.ds1302.halted),
            self.ds1302.sub_ns,
            ds1302_ram,
            at24c02
        );
        out
    }

    pub(crate) fn decode(text: &str) -> Result<Self> {
        let mut parts = text.split('|');
        let Some(prefix) = parts.next() else {
            bail!("持久状态字符串为空");
        };
        if prefix != PREFIX {
            bail!("不支持的持久状态版本: {prefix}");
        }

        let mut ds18b20 = None;
        let mut ds1302 = None;
        let mut ds1302_ram = None;
        let mut at24c02 = None;

        for section in parts {
            let Some((key, value)) = section.split_once('=') else {
                bail!("持久状态字段格式错误: {section}");
            };
            match key {
                "ds18b20" => ds18b20 = Some(parse_ds18b20(value)?),
                "ds1302" => ds1302 = Some(parse_ds1302(value)?),
                "ds1302_ram" => ds1302_ram = Some(parse_hex_array::<31>(value)?),
                "at24c02" => {
                    at24c02 = Some(At24c02PersistentState {
                        memory: parse_hex_array::<256>(value)?,
                    });
                }
                _ => bail!("未知持久状态字段: {key}"),
            }
        }

        let mut ds1302 = ds1302.ok_or_else(|| anyhow::anyhow!("缺少 ds1302 字段"))?;
        ds1302.ram = ds1302_ram.ok_or_else(|| anyhow::anyhow!("缺少 ds1302_ram 字段"))?;

        Ok(Self {
            ds18b20: ds18b20.ok_or_else(|| anyhow::anyhow!("缺少 ds18b20 字段"))?,
            ds1302,
            at24c02: at24c02.ok_or_else(|| anyhow::anyhow!("缺少 at24c02 字段"))?,
        })
    }
}

fn parse_ds18b20(value: &str) -> Result<Ds18b20PersistentState> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 4 {
        bail!("ds18b20 持久状态字段数量错误");
    }
    Ok(Ds18b20PersistentState {
        rom: parse_hex_array::<8>(fields[0])?,
        eeprom_th: parse_hex_byte(fields[1])?,
        eeprom_tl: parse_hex_byte(fields[2])?,
        eeprom_config: parse_hex_byte(fields[3])?,
    })
}

fn parse_ds1302(value: &str) -> Result<Ds1302PersistentState> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 12 {
        bail!("ds1302 持久状态字段数量错误");
    }

    let sub_ns = parse_hex_u64(fields[11])?;
    if sub_ns >= MAX_SUB_NS_EXCLUSIVE {
        bail!("ds1302 sub_ns 越界: {sub_ns}");
    }

    Ok(Ds1302PersistentState {
        write_protect: parse_bool01(fields[0])?,
        trickle_charge: parse_hex_byte(fields[1])?,
        ram: [0; 31],
        hour_mode_12: parse_bool01(fields[2])?,
        hour: parse_hex_byte(fields[3])?,
        minute: parse_hex_byte(fields[4])?,
        second: parse_hex_byte(fields[5])?,
        day_of_week: parse_hex_byte(fields[6])?,
        date: parse_hex_byte(fields[7])?,
        month: parse_hex_byte(fields[8])?,
        year: parse_hex_byte(fields[9])?,
        halted: parse_bool01(fields[10])?,
        sub_ns,
    })
}

fn parse_bool01(value: &str) -> Result<bool> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => bail!("布尔字段只能是 0 或 1: {value}"),
    }
}

fn parse_hex_byte(value: &str) -> Result<u8> {
    if value.len() != 2 {
        bail!("十六进制字节长度必须为 2: {value}");
    }
    u8::from_str_radix(value, 16).map_err(Into::into)
}

fn parse_hex_u64(value: &str) -> Result<u64> {
    u64::from_str_radix(value, 16).map_err(Into::into)
}

fn parse_hex_array<const N: usize>(value: &str) -> Result<[u8; N]> {
    if value.len() != N * 2 {
        bail!("十六进制字段长度错误, 期望 {} 实际 {}", N * 2, value.len());
    }
    let mut out = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk)?;
        out[index] = parse_hex_byte(text)?;
    }
    Ok(out)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02X}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        At24c02PersistentState, Ds18b20PersistentState, Ds1302PersistentState, PersistentState,
    };

    #[test]
    fn persistent_state_round_trips() {
        let state = PersistentState {
            ds18b20: Ds18b20PersistentState {
                rom: [0x28, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD],
                eeprom_th: 0x4B,
                eeprom_tl: 0x46,
                eeprom_config: 0x5F,
            },
            ds1302: Ds1302PersistentState {
                write_protect: true,
                trickle_charge: 0xA5,
                ram: [0x11; 31],
                hour_mode_12: false,
                hour: 12,
                minute: 34,
                second: 56,
                day_of_week: 7,
                date: 30,
                month: 6,
                year: 26,
                halted: true,
                sub_ns: 123_456_789,
            },
            at24c02: At24c02PersistentState {
                memory: [0x5A; 256],
            },
        };

        let encoded = state.encode();
        let decoded = PersistentState::decode(&encoded).expect("decode persistent state");
        assert_eq!(decoded, state);
    }
}

use anyhow::{Result, bail};

use crate::wave::{
    TRACK_EVENT_ADC_DAC, TRACK_EVENT_CPU, TRACK_EVENT_DS1302, TRACK_EVENT_I2C, TRACK_EVENT_ONEWIRE,
    TRACK_EVENT_SEG_CHANGE, TRACK_EVENT_UART1, TRACK_EVENT_UART2, seg_digit_change_track_id,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTrack {
    Cpu,
    I2c,
    Onewire,
    Uart1,
    Uart2,
    AdcDac,
    Ds1302,
    SegChange,
    SegDigit1,
    SegDigit2,
    SegDigit3,
    SegDigit4,
    SegDigit5,
    SegDigit6,
    SegDigit7,
    SegDigit8,
}

impl EventTrack {
    pub const COUNT: usize = 16;

    pub fn parse(name: &str) -> Result<Self> {
        let normalized = normalize_track_name(name);
        match normalized.as_str() {
            "cpu" | "event.cpu" => Ok(Self::Cpu),
            "i2c" | "iic" | "event.i2c" | "event.iic" => Ok(Self::I2c),
            "onewire" | "1wire" | "event.onewire" | "event.1wire" => Ok(Self::Onewire),
            "uart1" | "serial1" | "event.uart1" | "event.serial1" => Ok(Self::Uart1),
            "uart2" | "serial2" | "event.uart2" | "event.serial2" => Ok(Self::Uart2),
            "adc.dac" | "adcdac" | "event.adc.dac" | "event.adcdac" => Ok(Self::AdcDac),
            "ds1302" | "rtc" | "event.ds1302" | "event.rtc" => Ok(Self::Ds1302),
            "seg.change" | "event.seg.change" => Ok(Self::SegChange),
            "seg.d1.change" | "event.seg.d1.change" => Ok(Self::SegDigit1),
            "seg.d2.change" | "event.seg.d2.change" => Ok(Self::SegDigit2),
            "seg.d3.change" | "event.seg.d3.change" => Ok(Self::SegDigit3),
            "seg.d4.change" | "event.seg.d4.change" => Ok(Self::SegDigit4),
            "seg.d5.change" | "event.seg.d5.change" => Ok(Self::SegDigit5),
            "seg.d6.change" | "event.seg.d6.change" => Ok(Self::SegDigit6),
            "seg.d7.change" | "event.seg.d7.change" => Ok(Self::SegDigit7),
            "seg.d8.change" | "event.seg.d8.change" => Ok(Self::SegDigit8),
            _ => bail!("未知事件轨: {name}"),
        }
    }

    pub fn from_track_id(track_id: &str) -> Option<Self> {
        match track_id {
            TRACK_EVENT_CPU => Some(Self::Cpu),
            TRACK_EVENT_I2C => Some(Self::I2c),
            TRACK_EVENT_ONEWIRE => Some(Self::Onewire),
            TRACK_EVENT_UART1 => Some(Self::Uart1),
            TRACK_EVENT_UART2 => Some(Self::Uart2),
            TRACK_EVENT_ADC_DAC => Some(Self::AdcDac),
            TRACK_EVENT_DS1302 => Some(Self::Ds1302),
            TRACK_EVENT_SEG_CHANGE => Some(Self::SegChange),
            "event.seg.d1.change" => Some(Self::SegDigit1),
            "event.seg.d2.change" => Some(Self::SegDigit2),
            "event.seg.d3.change" => Some(Self::SegDigit3),
            "event.seg.d4.change" => Some(Self::SegDigit4),
            "event.seg.d5.change" => Some(Self::SegDigit5),
            "event.seg.d6.change" => Some(Self::SegDigit6),
            "event.seg.d7.change" => Some(Self::SegDigit7),
            "event.seg.d8.change" => Some(Self::SegDigit8),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Cpu => 0,
            Self::I2c => 1,
            Self::Onewire => 2,
            Self::Uart1 => 3,
            Self::Uart2 => 4,
            Self::AdcDac => 5,
            Self::Ds1302 => 6,
            Self::SegChange => 7,
            Self::SegDigit1 => 8,
            Self::SegDigit2 => 9,
            Self::SegDigit3 => 10,
            Self::SegDigit4 => 11,
            Self::SegDigit5 => 12,
            Self::SegDigit6 => 13,
            Self::SegDigit7 => 14,
            Self::SegDigit8 => 15,
        }
    }

    pub fn track_id(self) -> &'static str {
        match self {
            Self::Cpu => TRACK_EVENT_CPU,
            Self::I2c => TRACK_EVENT_I2C,
            Self::Onewire => TRACK_EVENT_ONEWIRE,
            Self::Uart1 => TRACK_EVENT_UART1,
            Self::Uart2 => TRACK_EVENT_UART2,
            Self::AdcDac => TRACK_EVENT_ADC_DAC,
            Self::Ds1302 => TRACK_EVENT_DS1302,
            Self::SegChange => TRACK_EVENT_SEG_CHANGE,
            Self::SegDigit1 => seg_digit_change_track_id(1),
            Self::SegDigit2 => seg_digit_change_track_id(2),
            Self::SegDigit3 => seg_digit_change_track_id(3),
            Self::SegDigit4 => seg_digit_change_track_id(4),
            Self::SegDigit5 => seg_digit_change_track_id(5),
            Self::SegDigit6 => seg_digit_change_track_id(6),
            Self::SegDigit7 => seg_digit_change_track_id(7),
            Self::SegDigit8 => seg_digit_change_track_id(8),
        }
    }

    pub fn seg_digit(digit: usize) -> Option<Self> {
        match digit {
            1 => Some(Self::SegDigit1),
            2 => Some(Self::SegDigit2),
            3 => Some(Self::SegDigit3),
            4 => Some(Self::SegDigit4),
            5 => Some(Self::SegDigit5),
            6 => Some(Self::SegDigit6),
            7 => Some(Self::SegDigit7),
            8 => Some(Self::SegDigit8),
            _ => None,
        }
    }
}

fn normalize_track_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    let mut last_was_sep = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_sep = false;
            continue;
        }
        if !last_was_sep {
            normalized.push('.');
            last_was_sep = true;
        }
    }
    normalized.trim_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::EventTrack;

    #[test]
    fn parse_event_track_supports_wave_track_aliases() {
        assert_eq!(EventTrack::parse("event.uart1").unwrap(), EventTrack::Uart1);
        assert_eq!(EventTrack::parse("event.iic").unwrap(), EventTrack::I2c);
        assert_eq!(EventTrack::parse("rtc").unwrap(), EventTrack::Ds1302);
        assert_eq!(
            EventTrack::parse("seg.d3.change").unwrap(),
            EventTrack::SegDigit3
        );
        assert_eq!(
            EventTrack::from_track_id(EventTrack::AdcDac.track_id()),
            Some(EventTrack::AdcDac)
        );
    }
}

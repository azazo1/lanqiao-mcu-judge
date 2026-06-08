use anyhow::{Result, anyhow, bail};

use crate::script::run_target::RunToTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardLatchSource {
    Effective,
    Port,
    Xdata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolStateTarget {
    Signal(RunToTarget),
    LatchBit {
        port: usize,
        bit: u8,
    },
    BoardBit {
        source: BoardLatchSource,
        slot: u8,
        bit: u8,
    },
    SegVisible {
        digit: u8,
    },
}

impl BoolStateTarget {
    pub fn parse(name: &str) -> Result<Self> {
        match StateTarget::parse(name)? {
            StateTarget::Bool(target) => Ok(target),
            StateTarget::Integer(_) | StateTarget::Text(_) => {
                bail!("状态目标 `{name}` 不是布尔类型")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntStateTarget {
    PinByte { port: usize },
    LatchByte { port: usize },
    BoardByte { source: BoardLatchSource, slot: u8 },
    SegRaw { digit: u8 },
    SegPattern { digit: u8 },
}

impl IntStateTarget {
    pub fn parse(name: &str) -> Result<Self> {
        match StateTarget::parse(name)? {
            StateTarget::Integer(target) => Ok(target),
            StateTarget::Bool(_) | StateTarget::Text(_) => {
                bail!("状态目标 `{name}` 不是整数类型")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStateTarget {
    SegText,
    SegDigitText { digit: u8 },
}

impl TextStateTarget {
    pub fn parse(name: &str) -> Result<Self> {
        match StateTarget::parse(name)? {
            StateTarget::Text(target) => Ok(target),
            StateTarget::Bool(_) | StateTarget::Integer(_) => {
                bail!("状态目标 `{name}` 不是文本类型")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTarget {
    Bool(BoolStateTarget),
    Integer(IntStateTarget),
    Text(TextStateTarget),
}

impl StateTarget {
    pub fn parse(name: &str) -> Result<Self> {
        if let Ok(target) = RunToTarget::parse(name) {
            return Ok(Self::Bool(BoolStateTarget::Signal(target)));
        }

        let normalized = normalize_state_name(name);
        let parts = normalized
            .split('.')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            bail!("状态目标不能为空");
        }

        match parts.as_slice() {
            ["pin", port] => Ok(Self::Integer(IntStateTarget::PinByte {
                port: parse_port_token(port)?,
            })),
            ["pin", port, bit] => Ok(Self::Bool(BoolStateTarget::Signal(RunToTarget::Pin {
                port: parse_port_token(port)?,
                bit: parse_bit_token(bit)?,
            }))),
            ["latch", port] => Ok(Self::Integer(IntStateTarget::LatchByte {
                port: parse_port_token(port)?,
            })),
            ["latch", port, bit] => Ok(Self::Bool(BoolStateTarget::LatchBit {
                port: parse_port_token(port)?,
                bit: parse_bit_token(bit)?,
            })),
            ["board", source, slot] => Ok(Self::Integer(IntStateTarget::BoardByte {
                source: parse_board_latch_source(source)?,
                slot: parse_board_slot_token(slot)?,
            })),
            ["board", source, slot, bit] => Ok(Self::Bool(BoolStateTarget::BoardBit {
                source: parse_board_latch_source(source)?,
                slot: parse_board_slot_token(slot)?,
                bit: parse_bit_token(bit)?,
            })),
            ["seg", "text"] => Ok(Self::Text(TextStateTarget::SegText)),
            ["seg", digit, "text"] => Ok(Self::Text(TextStateTarget::SegDigitText {
                digit: parse_digit_token(digit)?,
            })),
            ["seg", digit, "raw"] => Ok(Self::Integer(IntStateTarget::SegRaw {
                digit: parse_digit_token(digit)?,
            })),
            ["seg", digit, "pattern"] => Ok(Self::Integer(IntStateTarget::SegPattern {
                digit: parse_digit_token(digit)?,
            })),
            ["seg", digit, "scan"] | ["seg", digit, "enable"] | ["seg", digit, "on"] => {
                Ok(Self::Bool(BoolStateTarget::Signal(RunToTarget::SegDigit(
                    parse_digit_token(digit)?,
                ))))
            }
            ["seg", digit, "visible"] | ["seg", digit, "shown"] => {
                Ok(Self::Bool(BoolStateTarget::SegVisible {
                    digit: parse_digit_token(digit)?,
                }))
            }
            _ => bail!("未知状态目标: {name}"),
        }
    }
}

fn normalize_state_name(name: &str) -> String {
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

fn parse_port_token(token: &str) -> Result<usize> {
    let digits = token.strip_prefix('p').unwrap_or(token);
    let port = digits
        .parse::<usize>()
        .map_err(|_| anyhow!("非法端口编号: {token}"))?;
    if port > 5 {
        bail!("端口编号必须在 P0..P5");
    }
    Ok(port)
}

fn parse_bit_token(token: &str) -> Result<u8> {
    let bit = token
        .parse::<u8>()
        .map_err(|_| anyhow!("非法 bit 编号: {token}"))?;
    if bit > 7 {
        bail!("bit 编号必须在 0..=7");
    }
    Ok(bit)
}

fn parse_digit_token(token: &str) -> Result<u8> {
    let digits = token.strip_prefix('d').unwrap_or(token);
    let digit = digits
        .parse::<u8>()
        .map_err(|_| anyhow!("非法数码管编号: {token}"))?;
    if !(1..=8).contains(&digit) {
        bail!("数码管编号必须在 D1..D8");
    }
    Ok(digit)
}

fn parse_board_latch_source(token: &str) -> Result<BoardLatchSource> {
    match token {
        "effective" => Ok(BoardLatchSource::Effective),
        "port" => Ok(BoardLatchSource::Port),
        "xdata" => Ok(BoardLatchSource::Xdata),
        _ => bail!("未知板级锁存器来源: {token}"),
    }
}

fn parse_board_slot_token(token: &str) -> Result<u8> {
    let slot = match token {
        "led" => 0,
        "ctrl" | "control" => 1,
        "com" => 2,
        "seg" | "segment" => 3,
        _ => token
            .parse::<u8>()
            .map_err(|_| anyhow!("非法板级锁存器槽位: {token}"))?,
    };
    if slot > 3 {
        bail!("板级锁存器槽位必须在 0..=3");
    }
    Ok(slot)
}

#[cfg(test)]
mod tests {
    use super::{BoardLatchSource, BoolStateTarget, IntStateTarget, StateTarget, TextStateTarget};
    use crate::script::run_target::RunToTarget;

    #[test]
    fn parse_state_target_supports_wave_style_names() {
        assert_eq!(
            StateTarget::parse("pin.p3.4").unwrap(),
            StateTarget::Bool(BoolStateTarget::Signal(RunToTarget::Pin {
                port: 3,
                bit: 4
            }))
        );
        assert_eq!(
            StateTarget::parse("latch.p2.1").unwrap(),
            StateTarget::Bool(BoolStateTarget::LatchBit { port: 2, bit: 1 })
        );
        assert_eq!(
            StateTarget::parse("board.effective.com").unwrap(),
            StateTarget::Integer(IntStateTarget::BoardByte {
                source: BoardLatchSource::Effective,
                slot: 2,
            })
        );
        assert_eq!(
            StateTarget::parse("seg.d3.pattern").unwrap(),
            StateTarget::Integer(IntStateTarget::SegPattern { digit: 3 })
        );
        assert_eq!(
            StateTarget::parse("seg.d8.text").unwrap(),
            StateTarget::Text(TextStateTarget::SegDigitText { digit: 8 })
        );
        assert_eq!(
            StateTarget::parse("seg.d3.visible").unwrap(),
            StateTarget::Bool(BoolStateTarget::SegVisible { digit: 3 })
        );
    }
}

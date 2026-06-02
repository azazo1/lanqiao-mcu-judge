use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LedId {
    L1,
    L2,
    L3,
    L4,
    L5,
    L6,
    L7,
    L8,
}

impl LedId {
    pub fn index(self) -> usize {
        match self {
            Self::L1 => 1,
            Self::L2 => 2,
            Self::L3 => 3,
            Self::L4 => 4,
            Self::L5 => 5,
            Self::L6 => 6,
            Self::L7 => 7,
            Self::L8 => 8,
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "L1" => Ok(Self::L1),
            "L2" => Ok(Self::L2),
            "L3" => Ok(Self::L3),
            "L4" => Ok(Self::L4),
            "L5" => Ok(Self::L5),
            "L6" => Ok(Self::L6),
            "L7" => Ok(Self::L7),
            "L8" => Ok(Self::L8),
            _ => bail!("未知 LED: {name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyId {
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    S11,
    S12,
    S13,
    S14,
    S15,
    S16,
    S17,
    S18,
    S19,
}

impl KeyId {
    pub fn matrix_position(self) -> (usize, usize) {
        match self {
            Self::S4 => (3, 0),
            Self::S5 => (2, 0),
            Self::S6 => (1, 0),
            Self::S7 => (0, 0),
            Self::S8 => (3, 1),
            Self::S9 => (2, 1),
            Self::S10 => (1, 1),
            Self::S11 => (0, 1),
            Self::S12 => (3, 2),
            Self::S13 => (2, 2),
            Self::S14 => (1, 2),
            Self::S15 => (0, 2),
            Self::S16 => (3, 3),
            Self::S17 => (2, 3),
            Self::S18 => (1, 3),
            Self::S19 => (0, 3),
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "S4" => Ok(Self::S4),
            "S5" => Ok(Self::S5),
            "S6" => Ok(Self::S6),
            "S7" => Ok(Self::S7),
            "S8" => Ok(Self::S8),
            "S9" => Ok(Self::S9),
            "S10" => Ok(Self::S10),
            "S11" => Ok(Self::S11),
            "S12" => Ok(Self::S12),
            "S13" => Ok(Self::S13),
            "S14" => Ok(Self::S14),
            "S15" => Ok(Self::S15),
            "S16" => Ok(Self::S16),
            "S17" => Ok(Self::S17),
            "S18" => Ok(Self::S18),
            "S19" => Ok(Self::S19),
            _ => bail!("未知按键: {name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoltageChannel {
    Rb2,
    Rb3,
    Rb4,
    Rd1,
}

impl VoltageChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rb2 => "RB2",
            Self::Rb3 => "RB3",
            Self::Rb4 => "RB4",
            Self::Rd1 => "RD1",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "RB2" => Ok(Self::Rb2),
            "RB3" => Ok(Self::Rb3),
            "RB4" => Ok(Self::Rb4),
            "RD1" => Ok(Self::Rd1),
            _ => bail!("未知电压通道: {name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyMode {
    #[default]
    Keyboard,
    Button,
}

impl KeyMode {
    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "keyboard" | "matrix" | "matrix_keypad" | "kbd" => Ok(Self::Keyboard),
            "button" | "buttons" | "independent" | "independent_button" | "btn" => {
                Ok(Self::Button)
            }
            _ => bail!("未知按键模式: {name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SignalId {
    SigOut,
    NetSig,
}

impl SignalId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SigOut => "SIG_OUT",
            Self::NetSig => "NET_SIG",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "SIG_OUT" | "SIGOUT" => Ok(Self::SigOut),
            "NET_SIG" | "NETSIG" => Ok(Self::NetSig),
            _ => bail!("未知跳帽信号: {name}"),
        }
    }
}

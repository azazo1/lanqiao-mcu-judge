use std::path::Path;

use anyhow::Result;

use crate::{
    chip::Simulator,
    script::run_target::{RunToEdge, RunToTarget},
};

const SFR_P1_ADDR: u8 = 0x90;
const SFR_P2_ADDR: u8 = 0xA0;

pub struct BenchHarness {
    sim: Simulator,
}

impl BenchHarness {
    pub fn nop() -> Self {
        Self {
            sim: Simulator::nop(false),
        }
    }

    pub fn from_code(code: Vec<u8>) -> Self {
        Self::from_code_with_options(code, crate::wave::WaveCaptureOptions::default())
    }

    pub fn from_code_with_options(
        code: Vec<u8>,
        wave_options: crate::wave::WaveCaptureOptions,
    ) -> Self {
        Self {
            sim: Simulator::from_code_with_options(code, false, wave_options),
        }
    }

    pub fn from_hex_path(path: &Path) -> Result<Self> {
        Ok(Self {
            sim: Simulator::from_hex_path(path, false)?,
        })
    }

    pub fn reset(&mut self) -> Result<()> {
        self.sim.reset()
    }

    pub fn run_ms(&mut self, ms: u64) -> Result<()> {
        self.sim.run_ms(ms)
    }

    pub fn run_us(&mut self, us: u64) -> Result<()> {
        self.sim.run_us(us)
    }

    pub fn run_to_ns(&mut self, target_ns: u64) -> Result<u64> {
        self.sim.run_to_ns(target_ns)
    }

    pub fn run_to_pin_up(&mut self, port: usize, bit: u8) -> Result<u64> {
        self.sim
            .run_to_target(RunToTarget::Pin { port, bit }, RunToEdge::Up)
    }

    pub fn run_to_pin_flip(&mut self, port: usize, bit: u8) -> Result<u64> {
        self.sim
            .run_to_target(RunToTarget::Pin { port, bit }, RunToEdge::Flip)
    }

    pub fn set_key(&mut self, name: &str, pressed: bool) -> Result<()> {
        self.sim.set_key(name, pressed)
    }

    pub fn tap_key(&mut self, name: &str, hold_ms: u64) -> Result<()> {
        self.sim.tap_key(name, hold_ms)
    }

    pub fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        self.sim.set_voltage(name, value)
    }

    pub fn poke_sfr(&mut self, addr: u8, value: u8) -> Result<()> {
        self.sim.poke_sfr(addr, value)
    }

    pub fn uart_write(&mut self, bytes: &[u8]) -> Result<()> {
        self.sim.uart_write(bytes)
    }

    pub fn uart_take_string(&mut self) -> Result<String> {
        self.sim.uart_take_string()
    }

    pub fn display_text(&self) -> String {
        self.sim.display_text()
    }

    pub fn snapshot_text(&self) -> String {
        self.sim.snapshot_text()
    }

    pub fn sim_time_ns(&self) -> u64 {
        self.sim.sim_time_ns()
    }

    pub fn i2c_idle(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P2_ADDR, 0xFF)
    }

    pub fn i2c_start(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P2_ADDR, 0xFF)?;
        self.run_us(10)?;
        self.poke_sfr(SFR_P2_ADDR, 0xFD)?;
        self.run_us(10)?;
        self.poke_sfr(SFR_P2_ADDR, 0xFC)?;
        self.run_us(10)
    }

    pub fn i2c_stop(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P2_ADDR, 0xFC)?;
        self.run_us(10)?;
        self.poke_sfr(SFR_P2_ADDR, 0xFD)?;
        self.run_us(10)?;
        self.poke_sfr(SFR_P2_ADDR, 0xFF)?;
        self.run_us(10)
    }

    pub fn i2c_write_byte(&mut self, byte: u8) -> Result<()> {
        let mut value = byte;
        for _ in 0..8 {
            let sda_high = value & 0x80 != 0;
            let port = if sda_high { 0xFD } else { 0xFC };
            self.poke_sfr(SFR_P2_ADDR, port)?;
            self.run_us(5)?;
            self.poke_sfr(SFR_P2_ADDR, port | 0x01)?;
            self.run_us(10)?;
            self.poke_sfr(SFR_P2_ADDR, port)?;
            self.run_us(5)?;
            value <<= 1;
        }
        self.poke_sfr(SFR_P2_ADDR, 0xFE)?;
        self.run_us(5)?;
        self.poke_sfr(SFR_P2_ADDR, 0xFF)?;
        self.run_us(10)?;
        self.poke_sfr(SFR_P2_ADDR, 0xFE)?;
        self.run_us(5)
    }

    pub fn onewire_idle(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P1_ADDR, 0xFF)
    }

    pub fn onewire_reset(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P1_ADDR, 0xFF)?;
        self.run_us(20)?;
        self.poke_sfr(SFR_P1_ADDR, 0xEF)?;
        self.run_us(500)?;
        self.poke_sfr(SFR_P1_ADDR, 0xFF)?;
        self.run_us(280)
    }

    pub fn onewire_write_bit(&mut self, bit: bool) -> Result<()> {
        self.poke_sfr(SFR_P1_ADDR, 0xEF)?;
        self.run_us(if bit { 5 } else { 65 })?;
        self.poke_sfr(SFR_P1_ADDR, 0xFF)?;
        self.run_us(10)
    }

    pub fn onewire_write_byte(&mut self, byte: u8) -> Result<()> {
        for bit in 0..8 {
            self.onewire_write_bit(byte & (1 << bit) != 0)?;
        }
        Ok(())
    }

    pub fn ds1302_idle(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P1_ADDR, 0xFF)?;
        self.poke_sfr(SFR_P2_ADDR, 0xFF)
    }

    pub fn ds1302_begin(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P1_ADDR, 0x7F)?;
        self.run_us(5)?;
        self.poke_sfr(SFR_P1_ADDR, 0xFF)?;
        self.run_us(5)
    }

    pub fn ds1302_end(&mut self) -> Result<()> {
        self.poke_sfr(SFR_P1_ADDR, 0x7F)?;
        self.run_us(5)
    }

    pub fn ds1302_write_byte(&mut self, byte: u8) -> Result<()> {
        let mut value = byte;
        for _ in 0..8 {
            let io_high = value & 0x01 != 0;
            let p2 = if io_high { 0xFF } else { 0xF7 };
            self.poke_sfr(SFR_P1_ADDR, 0x7F)?;
            self.poke_sfr(SFR_P2_ADDR, p2)?;
            self.run_us(5)?;
            self.poke_sfr(SFR_P1_ADDR, 0xFF)?;
            self.run_us(10)?;
            self.poke_sfr(SFR_P1_ADDR, 0x7F)?;
            self.run_us(5)?;
            value >>= 1;
        }
        Ok(())
    }
}

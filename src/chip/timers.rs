use anyhow::{Result, bail};

use super::{CPU_TICKS_PER_US, registers::*};

#[derive(Debug, Clone, Copy)]
pub(crate) struct TimerSnapshot {
    pub(crate) tcon: u8,
    pub(crate) tmod: u8,
    pub(crate) tl0: u8,
    pub(crate) th0: u8,
    pub(crate) tl1: u8,
    pub(crate) th1: u8,
    pub(crate) t2h: u8,
    pub(crate) t2l: u8,
    pub(crate) cmod: u8,
    pub(crate) ccon: u8,
    pub(crate) ch: u8,
    pub(crate) cl: u8,
}

#[derive(Debug, Default)]
pub(crate) struct TimerBlock {
    timer01: Timer01,
    timer2: Timer2,
    pca: Pca,
}

impl TimerBlock {
    pub(crate) fn handles(addr: u8) -> bool {
        matches!(
            addr,
            SFR_TCON | SFR_TMOD | SFR_TL0 | SFR_TL1 | SFR_TH0 | SFR_TH1 | SFR_T2H | SFR_T2L
        )
    }

    pub(crate) fn read(&self, generic: &[u8; 128], addr: u8) -> Option<u8> {
        match addr {
            SFR_TCON | SFR_TMOD | SFR_TL0 | SFR_TL1 | SFR_TH0 | SFR_TH1 => {
                Some(self.timer01.read(addr))
            }
            SFR_T2H | SFR_T2L => Some(read_sfr(generic, addr)),
            _ => None,
        }
    }

    pub(crate) fn write(&mut self, generic: &mut [u8; 128], addr: u8, value: u8) -> bool {
        match addr {
            SFR_TCON | SFR_TMOD | SFR_TL0 | SFR_TL1 | SFR_TH0 | SFR_TH1 => {
                self.timer01.write(addr, value);
                true
            }
            SFR_T2H | SFR_T2L => {
                self.timer2.write(generic, addr, value);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn tick_timers01_t2(
        &mut self,
        p3: u8,
        auxr: u8,
        ticks: u32,
        generic: &mut [u8; 128],
    ) -> Result<()> {
        self.timer01.tick(p3, auxr, ticks)?;
        self.timer2.tick(p3, auxr, ticks, generic)?;
        Ok(())
    }

    pub(crate) fn tick_pca(&mut self, ticks: u32, generic: &mut [u8; 128]) -> Result<()> {
        self.pca.tick(ticks, generic)
    }

    pub(crate) fn snapshot(&self, generic: &[u8; 128]) -> TimerSnapshot {
        TimerSnapshot {
            tcon: self.timer01.tcon,
            tmod: self.timer01.tmod,
            tl0: self.timer01.tl0,
            th0: self.timer01.th0,
            tl1: self.timer01.tl1,
            th1: self.timer01.th1,
            t2h: read_sfr(generic, SFR_T2H),
            t2l: read_sfr(generic, SFR_T2L),
            cmod: read_sfr(generic, SFR_CMOD),
            ccon: read_sfr(generic, SFR_CCON),
            ch: read_sfr(generic, SFR_CH),
            cl: read_sfr(generic, SFR_CL),
        }
    }
}

#[derive(Debug, Default)]
struct Timer01 {
    tcon: u8,
    tmod: u8,
    tl0: u8,
    tl1: u8,
    th0: u8,
    th1: u8,
    rl_tl0: u8,
    rl_tl1: u8,
    rl_th0: u8,
    rl_th1: u8,
    div0: u8,
    div1: u8,
    prev_p3: u8,
}

impl Timer01 {
    fn read(&self, addr: u8) -> u8 {
        match addr {
            SFR_TCON => self.tcon,
            SFR_TMOD => self.tmod,
            SFR_TL0 => self.tl0,
            SFR_TL1 => self.tl1,
            SFR_TH0 => self.th0,
            SFR_TH1 => self.th1,
            _ => 0,
        }
    }

    fn write(&mut self, addr: u8, value: u8) {
        match addr {
            SFR_TCON => self.tcon = value,
            SFR_TMOD => self.tmod = value,
            SFR_TL0 => {
                self.tl0 = value;
                self.rl_tl0 = value;
            }
            SFR_TL1 => {
                self.tl1 = value;
                self.rl_tl1 = value;
            }
            SFR_TH0 => {
                self.th0 = value;
                self.rl_th0 = value;
            }
            SFR_TH1 => {
                self.th1 = value;
                self.rl_th1 = value;
            }
            _ => {}
        }
    }

    fn tick(&mut self, p3: u8, auxr: u8, ticks: u32) -> Result<()> {
        self.tick_timer0(p3, auxr, ticks)?;
        self.tick_timer1(p3, auxr, ticks);
        self.prev_p3 = p3;
        Ok(())
    }

    fn tick_timer0(&mut self, p3: u8, auxr: u8, ticks: u32) -> Result<()> {
        if self.tcon & TCON_TR0 == 0 {
            self.div0 = 0;
            return Ok(());
        }
        if self.tmod & TMOD_GATE0 != 0 && p3 & P3_INT0 == 0 {
            self.div0 = 0;
            return Ok(());
        }

        let tick_count = if self.tmod & TMOD_C_T0 != 0 {
            u32::from(self.prev_p3 & P3_T0 != 0 && p3 & P3_T0 == 0)
        } else {
            timer_tick_count(auxr & AUXR_T0_X12 != 0, &mut self.div0, ticks)
        };
        if tick_count == 0 {
            return Ok(());
        }

        match self.tmod & 0x03 {
            0x00 => {
                for _ in 0..tick_count {
                    let next = u16::from_be_bytes([self.th0, self.tl0]).wrapping_add(1);
                    if next == 0 {
                        self.th0 = self.rl_th0;
                        self.tl0 = self.rl_tl0;
                        self.tcon |= TCON_TF0;
                    } else {
                        let [th0, tl0] = next.to_be_bytes();
                        self.th0 = th0;
                        self.tl0 = tl0;
                    }
                }
                Ok(())
            }
            0x01 => {
                for _ in 0..tick_count {
                    let next = u16::from_be_bytes([self.th0, self.tl0]).wrapping_add(1);
                    let [th0, tl0] = next.to_be_bytes();
                    self.th0 = th0;
                    self.tl0 = tl0;
                    if next == 0 {
                        self.tcon |= TCON_TF0;
                    }
                }
                Ok(())
            }
            0x02 => {
                for _ in 0..tick_count {
                    self.tl0 = self.tl0.wrapping_add(1);
                    if self.tl0 == 0 {
                        self.tl0 = self.th0;
                        self.tcon |= TCON_TF0;
                    }
                }
                Ok(())
            }
            0x03 => bail!("暂不支持定时器0模式3: STC15 该模式为不可屏蔽中断的16位自动重装载"),
            _ => unreachable!(),
        }
    }

    fn tick_timer1(&mut self, p3: u8, auxr: u8, ticks: u32) {
        if self.tcon & TCON_TR1 == 0 {
            self.div1 = 0;
            return;
        }
        if self.tmod & TMOD_GATE1 != 0 && p3 & P3_INT1 == 0 {
            self.div1 = 0;
            return;
        }

        let mode = (self.tmod >> 4) & 0x03;
        if mode == 0x03 {
            self.div1 = 0;
            return;
        }

        let tick_count = if self.tmod & TMOD_C_T1 != 0 {
            u32::from(self.prev_p3 & P3_T1 != 0 && p3 & P3_T1 == 0)
        } else {
            timer_tick_count(auxr & AUXR_T1_X12 != 0, &mut self.div1, ticks)
        };
        if tick_count == 0 {
            return;
        }

        match mode {
            0x00 => {
                for _ in 0..tick_count {
                    let next = u16::from_be_bytes([self.th1, self.tl1]).wrapping_add(1);
                    if next == 0 {
                        self.th1 = self.rl_th1;
                        self.tl1 = self.rl_tl1;
                        self.tcon |= TCON_TF1;
                    } else {
                        let [th1, tl1] = next.to_be_bytes();
                        self.th1 = th1;
                        self.tl1 = tl1;
                    }
                }
            }
            0x01 => {
                for _ in 0..tick_count {
                    let next = u16::from_be_bytes([self.th1, self.tl1]).wrapping_add(1);
                    let [th1, tl1] = next.to_be_bytes();
                    self.th1 = th1;
                    self.tl1 = tl1;
                    if next == 0 {
                        self.tcon |= TCON_TF1;
                    }
                }
            }
            0x02 => {
                for _ in 0..tick_count {
                    self.tl1 = self.tl1.wrapping_add(1);
                    if self.tl1 == 0 {
                        self.tl1 = self.th1;
                        self.tcon |= TCON_TF1;
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default)]
struct Timer2 {
    reload_high: u8,
    reload_low: u8,
    divider: u8,
    prev_p3: u8,
}

impl Timer2 {
    fn write(&mut self, generic: &mut [u8; 128], addr: u8, value: u8) {
        write_sfr(generic, addr, value);
        match addr {
            SFR_T2H => self.reload_high = value,
            SFR_T2L => self.reload_low = value,
            _ => {}
        }
    }

    fn tick(&mut self, p3: u8, auxr: u8, ticks: u32, generic: &mut [u8; 128]) -> Result<()> {
        if auxr & AUXR_T2_RUN == 0 {
            self.divider = 0;
            self.prev_p3 = p3;
            return Ok(());
        }

        let tick_count = if auxr & AUXR_T2_C_T != 0 {
            u32::from(self.prev_p3 & P3_T2 != 0 && p3 & P3_T2 == 0)
        } else {
            timer_tick_count(auxr & AUXR_T2_X12 != 0, &mut self.divider, ticks)
        };
        if tick_count == 0 {
            self.prev_p3 = p3;
            return Ok(());
        }

        for _ in 0..tick_count {
            let next =
                u16::from_be_bytes([read_sfr(generic, SFR_T2H), read_sfr(generic, SFR_T2L)])
                    .wrapping_add(1);
            if next == 0 {
                write_sfr(generic, SFR_T2H, self.reload_high);
                write_sfr(generic, SFR_T2L, self.reload_low);
            } else {
                let [t2h, t2l] = next.to_be_bytes();
                write_sfr(generic, SFR_T2H, t2h);
                write_sfr(generic, SFR_T2L, t2l);
            }
        }
        self.prev_p3 = p3;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct Pca {
    divider: u64,
}

impl Pca {
    fn tick(&mut self, ticks: u32, generic: &mut [u8; 128]) -> Result<()> {
        if read_sfr(generic, SFR_CCON) & CCON_CR == 0 {
            self.divider = 0;
            return Ok(());
        }

        let cmod = read_sfr(generic, SFR_CMOD);
        if cmod != 0 {
            bail!(
                "暂不支持 PCA CMOD={cmod:02X}; 当前仅支持 CMOD=00, 即 SYSclk/12 且不启用额外选项"
            );
        }

        self.divider = self.divider.saturating_add(u64::from(ticks));
        if self.divider < CPU_TICKS_PER_US {
            return Ok(());
        }
        let increments = self.divider / CPU_TICKS_PER_US;
        self.divider %= CPU_TICKS_PER_US;

        for _ in 0..increments {
            let counter =
                u16::from_be_bytes([read_sfr(generic, SFR_CH), read_sfr(generic, SFR_CL)])
                    .wrapping_add(1);
            let [ch, cl] = counter.to_be_bytes();
            write_sfr(generic, SFR_CH, ch);
            write_sfr(generic, SFR_CL, cl);
            if counter == 0 {
                write_sfr(generic, SFR_CCON, read_sfr(generic, SFR_CCON) | CCON_CF);
            }
        }
        Ok(())
    }
}

fn read_sfr(generic: &[u8; 128], addr: u8) -> u8 {
    generic[usize::from(addr.wrapping_sub(0x80))]
}

fn write_sfr(generic: &mut [u8; 128], addr: u8, value: u8) {
    generic[usize::from(addr.wrapping_sub(0x80))] = value;
}

fn timer_tick_count(one_t: bool, divider: &mut u8, ticks: u32) -> u32 {
    if one_t {
        *divider = 0;
        return ticks;
    }

    let total = u32::from(*divider).saturating_add(ticks);
    *divider = (total % 12) as u8;
    total / 12
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::super::registers::*;
    use super::{TimerBlock, read_sfr, write_sfr};

    fn generic() -> [u8; 128] {
        [0; 128]
    }

    #[test]
    fn timer0_mode0_auto_reload_sets_tf0() -> Result<()> {
        let mut timers = TimerBlock::default();
        let mut generic = generic();
        assert!(timers.write(&mut generic, SFR_TH0, 0x12));
        assert!(timers.write(&mut generic, SFR_TL0, 0x34));
        assert!(timers.write(&mut generic, SFR_TCON, TCON_TR0));
        assert!(timers.write(&mut generic, SFR_TMOD, 0x00));
        assert!(timers.write(&mut generic, SFR_TH0, 0xFF));
        assert!(timers.write(&mut generic, SFR_TL0, 0xFF));

        timers.tick_timers01_t2(0xFF, AUXR_T0_X12, 1, &mut generic)?;

        let snapshot = timers.snapshot(&generic);
        assert_eq!(snapshot.th0, 0xFF);
        assert_eq!(snapshot.tl0, 0xFF);
        assert_eq!(snapshot.tcon & TCON_TF0, TCON_TF0);
        Ok(())
    }

    #[test]
    fn timer1_mode2_reloads_from_th1() -> Result<()> {
        let mut timers = TimerBlock::default();
        let mut generic = generic();
        assert!(timers.write(&mut generic, SFR_TH1, 0xA5));
        assert!(timers.write(&mut generic, SFR_TL1, 0xFF));
        assert!(timers.write(&mut generic, SFR_TMOD, 0x20));
        assert!(timers.write(&mut generic, SFR_TCON, TCON_TR1));

        timers.tick_timers01_t2(0xFF, AUXR_T1_X12, 1, &mut generic)?;

        let snapshot = timers.snapshot(&generic);
        assert_eq!(snapshot.tl1, 0xA5);
        assert_eq!(snapshot.tcon & TCON_TF1, TCON_TF1);
        Ok(())
    }

    #[test]
    fn timer2_auto_reload_uses_shadow_value() -> Result<()> {
        let mut timers = TimerBlock::default();
        let mut generic = generic();
        assert!(timers.write(&mut generic, SFR_T2H, 0x12));
        assert!(timers.write(&mut generic, SFR_T2L, 0x34));
        write_sfr(&mut generic, SFR_T2H, 0xFF);
        write_sfr(&mut generic, SFR_T2L, 0xFF);

        timers.tick_timers01_t2(0xFF, AUXR_T2_RUN | AUXR_T2_X12, 1, &mut generic)?;

        assert_eq!(read_sfr(&generic, SFR_T2H), 0x12);
        assert_eq!(read_sfr(&generic, SFR_T2L), 0x34);
        Ok(())
    }

    #[test]
    fn timer0_mode3_returns_error() -> Result<()> {
        let mut timers = TimerBlock::default();
        let mut generic = generic();
        assert!(timers.write(&mut generic, SFR_TMOD, 0x03));
        assert!(timers.write(&mut generic, SFR_TCON, TCON_TR0));

        let err = timers
            .tick_timers01_t2(0xFF, AUXR_T0_X12, 1, &mut generic)
            .expect_err("mode3 should fail");
        assert!(err.to_string().contains("模式3"));
        Ok(())
    }

    #[test]
    fn pca_counts_only_when_cmod_is_zero() -> Result<()> {
        let mut timers = TimerBlock::default();
        let mut generic = generic();
        write_sfr(&mut generic, SFR_CCON, CCON_CR);

        timers.tick_pca(super::super::CPU_TICKS_PER_US as u32, &mut generic)?;
        assert_eq!(read_sfr(&generic, SFR_CL), 1);

        write_sfr(&mut generic, SFR_CMOD, 0x01);
        let err = timers
            .tick_pca(1, &mut generic)
            .expect_err("non-zero CMOD should fail");
        assert!(err.to_string().contains("CMOD=00"));
        Ok(())
    }
}

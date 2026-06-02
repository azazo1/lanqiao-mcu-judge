#![allow(dead_code)]

// Core 8051 SFRs plus the STC15 series extensions referenced by the user
// manual. Some names are family-wide aliases that share the same address.

pub const SFR_P0: u8 = 0x80;
pub const SFR_SP: u8 = 0x81;
pub const SFR_DPL: u8 = 0x82;
pub const SFR_DPH: u8 = 0x83;
pub const SFR_S4CON: u8 = 0x84;
pub const SFR_S4BUF: u8 = 0x85;
pub const SFR_PCON: u8 = 0x87;

pub const SFR_TCON: u8 = 0x88;
pub const SFR_TMOD: u8 = 0x89;
pub const SFR_TL0: u8 = 0x8A;
pub const SFR_TL1: u8 = 0x8B;
pub const SFR_TH0: u8 = 0x8C;
pub const SFR_TH1: u8 = 0x8D;
pub const SFR_AUXR: u8 = 0x8E;
pub const SFR_INT_CLKO: u8 = 0x8F;
pub const SFR_AUXR2: u8 = SFR_INT_CLKO;

pub const SFR_P1: u8 = 0x90;
pub const SFR_P1M1: u8 = 0x91;
pub const SFR_P1M0: u8 = 0x92;
pub const SFR_P0M1: u8 = 0x93;
pub const SFR_P0M0: u8 = 0x94;
pub const SFR_P2M1: u8 = 0x95;
pub const SFR_P2M0: u8 = 0x96;
pub const SFR_CLK_DIV: u8 = 0x97;
pub const SFR_PCON2: u8 = SFR_CLK_DIV;
pub const SFR_SCON: u8 = 0x98;
pub const SFR_SBUF: u8 = 0x99;
pub const SFR_S2CON: u8 = 0x9A;
pub const SFR_S2BUF: u8 = 0x9B;
pub const SFR_P1ASF: u8 = 0x9D;

pub const SFR_P2: u8 = 0xA0;
pub const SFR_BUS_SPEED: u8 = 0xA1;
pub const SFR_AUXR1: u8 = 0xA2;
pub const SFR_P_SW1: u8 = 0xA2;
pub const SFR_WKTCL: u8 = 0xAA;
pub const SFR_WKTCH: u8 = 0xAB;
pub const SFR_S3CON: u8 = 0xAC;
pub const SFR_S3BUF: u8 = 0xAD;
pub const SFR_IE2: u8 = 0xAF;

pub const SFR_P3: u8 = 0xB0;
pub const SFR_P3M1: u8 = 0xB1;
pub const SFR_P3M0: u8 = 0xB2;
pub const SFR_P4M1: u8 = 0xB3;
pub const SFR_P4M0: u8 = 0xB4;
pub const SFR_IP2: u8 = 0xB5;
pub const SFR_IP2H: u8 = 0xB6;
pub const SFR_IPH: u8 = 0xB7;
pub const SFR_IP: u8 = 0xB8;
pub const SFR_SADEN: u8 = 0xB9;
pub const SFR_P_SW2: u8 = 0xBA;
pub const SFR_ADC_CONTR: u8 = 0xBC;
pub const SFR_ADC_RES: u8 = 0xBD;
pub const SFR_ADC_RESL: u8 = 0xBE;
pub const SFR_ADC_LOW2: u8 = SFR_ADC_RESL;

pub const SFR_P4: u8 = 0xC0;
pub const SFR_WDT_CONTR: u8 = 0xC1;
pub const SFR_IAP_DATA: u8 = 0xC2;
pub const SFR_IAP_ADDRH: u8 = 0xC3;
pub const SFR_IAP_ADDRL: u8 = 0xC4;
pub const SFR_IAP_CMD: u8 = 0xC5;
pub const SFR_IAP_TRIG: u8 = 0xC6;
pub const SFR_IAP_CONTR: u8 = 0xC7;
pub const SFR_P5: u8 = 0xC8;
pub const SFR_P5M1: u8 = 0xC9;
pub const SFR_P5M0: u8 = 0xCA;
pub const SFR_P6M1: u8 = 0xCB;
pub const SFR_P6M0: u8 = 0xCC;
pub const SFR_SPSTAT: u8 = 0xCD;
pub const SFR_SPCTL: u8 = 0xCE;
pub const SFR_SPDAT: u8 = 0xCF;

pub const SFR_PSW: u8 = 0xD0;
pub const SFR_T4T3M: u8 = 0xD1;
pub const SFR_T4H: u8 = 0xD2;
pub const SFR_T4L: u8 = 0xD3;
pub const SFR_T3H: u8 = 0xD4;
pub const SFR_T3L: u8 = 0xD5;
pub const SFR_T2H: u8 = 0xD6;
pub const SFR_T2L: u8 = 0xD7;
pub const SFR_CCON: u8 = 0xD8;
pub const SFR_CMOD: u8 = 0xD9;
pub const SFR_CCAPM0: u8 = 0xDA;
pub const SFR_CCAPM1: u8 = 0xDB;
pub const SFR_CCAPM2: u8 = 0xDC;
pub const SFR_CCAPM3: u8 = 0xDD;
pub const SFR_CCAPM4: u8 = 0xDE;

pub const SFR_ACC: u8 = 0xE0;
pub const SFR_P6: u8 = 0xE8;
pub const SFR_CL: u8 = 0xE9;
pub const SFR_CCAP0L: u8 = 0xEA;
pub const SFR_CCAP1L: u8 = 0xEB;
pub const SFR_CCAP2L: u8 = 0xEC;
pub const SFR_CCAP3L: u8 = 0xED;
pub const SFR_CCAP4L: u8 = 0xEE;
pub const SFR_P7M0: u8 = 0xEF;

pub const SFR_B: u8 = 0xF0;
pub const SFR_P7: u8 = 0xF8;
pub const SFR_CH: u8 = 0xF9;
pub const SFR_CCAP0H: u8 = 0xFA;
pub const SFR_CCAP1H: u8 = 0xFB;
pub const SFR_CCAP2H: u8 = 0xFC;
pub const SFR_CCAP3H: u8 = 0xFD;
pub const SFR_CCAP4H: u8 = 0xFE;
pub const SFR_P7M1: u8 = 0xFF;

pub const UART1_SFR_SCON: u8 = SFR_SCON;
pub const UART1_SFR_SBUF: u8 = SFR_SBUF;
pub const UART2_SFR_S2CON: u8 = SFR_S2CON;
pub const UART2_SFR_S2BUF: u8 = SFR_S2BUF;

pub const P3_INT0: u8 = 1 << 2;
pub const P3_INT1: u8 = 1 << 3;
pub const P3_T0: u8 = 1 << 4;
pub const P3_T1: u8 = 1 << 5;
pub const P3_T2: u8 = 1 << 1;

pub const TCON_TR0: u8 = 1 << 4;
pub const TCON_TF0: u8 = 1 << 5;
pub const TCON_TR1: u8 = 1 << 6;
pub const TCON_TF1: u8 = 1 << 7;

pub const TMOD_GATE0: u8 = 1 << 3;
pub const TMOD_C_T0: u8 = 1 << 2;
pub const TMOD_GATE1: u8 = 1 << 7;
pub const TMOD_C_T1: u8 = 1 << 6;

pub const SCON_REN: u8 = 1 << 4;
pub const SCON_TI: u8 = 1 << 1;
pub const SCON_RI: u8 = 1 << 0;

pub const S2CON_REN: u8 = 1 << 4;
pub const S2CON_TI: u8 = 1 << 1;
pub const S2CON_RI: u8 = 1 << 0;

pub const AUXR_T0_X12: u8 = 1 << 7;
pub const AUXR_T1_X12: u8 = 1 << 6;
pub const AUXR_UART_M0_X6: u8 = 1 << 5;
pub const AUXR_T2_RUN: u8 = 1 << 4;
pub const AUXR_T2_C_T: u8 = 1 << 3;
pub const AUXR_T2_X12: u8 = 1 << 2;
pub const AUXR_EXTRAM: u8 = 1 << 1;
pub const AUXR_S1ST2: u8 = 1 << 0;

pub const CCON_CR: u8 = 1 << 6;
pub const CCON_CF: u8 = 1 << 7;

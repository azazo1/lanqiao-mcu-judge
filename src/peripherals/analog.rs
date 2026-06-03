use std::collections::BTreeMap;

use anyhow::Result;

use crate::ids::VoltageChannel;

#[derive(Debug, Default)]
pub(crate) struct AnalogInputs {
    voltages: BTreeMap<String, f32>,
}

impl AnalogInputs {
    pub(crate) fn set_voltage(&mut self, name: &str, value: f32) -> Result<()> {
        self.set_voltage_channel(VoltageChannel::parse(name)?, value);
        Ok(())
    }

    pub(crate) fn set_voltage_channel(&mut self, channel: VoltageChannel, value: f32) {
        self.voltages
            .insert(channel.as_str().to_string(), value.clamp(0.0, 5.0));
    }

    pub(crate) fn channel_voltage(&self, channel: u8) -> f32 {
        match channel {
            1 => *self.voltages.get("RD1").unwrap_or(&0.0),
            3 => *self.voltages.get("RB2").unwrap_or(&0.0),
            _ => 0.0,
        }
    }

    pub(crate) fn channel_value(&self, channel: u8) -> u8 {
        ((self.channel_voltage(channel) / 5.0) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    }
}

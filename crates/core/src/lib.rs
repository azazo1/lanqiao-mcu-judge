mod app;
pub mod bench;
mod chip;
mod event;
mod hex;
mod ids;
mod jumper;
mod peripherals;
mod persistent_state;
mod script;
mod wave;

use anyhow::Result;

pub use bench::BenchHarness;
pub use chip::{
    CPU_EXEC_HZ, DisplayNumber, NS_PER_MICROSECOND, NS_PER_MILLISECOND, NS_PER_SECOND,
    SYSTEM_HZ, Simulator, UartConfig, UartParity, UartStopBits,
};
pub use ids::{KeyId, KeyMode, LedId, ResetMode, SignalId, VoltageChannel};
pub use peripherals::Ds1302State;
pub use script::{run_repl, run_script, run_script_source, run_script_stdin};
pub use script::run_target::{RunToEdge, RunToTarget};
pub use wave::WaveCaptureOptions;

pub async fn run_cli() -> Result<()> {
    app::run().await
}

mod app;
mod chip;
mod hex;
mod ids;
mod jumper;
mod peripherals;
mod persistent_state;
mod script;
mod script_target;
mod wave;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

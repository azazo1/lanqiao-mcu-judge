mod app;
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

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

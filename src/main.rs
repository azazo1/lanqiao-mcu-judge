mod app;
mod chip;
mod hex;
mod ids;
mod jumper;
mod peripherals;
mod persistent_state;
mod script;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

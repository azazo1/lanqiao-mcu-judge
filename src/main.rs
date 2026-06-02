mod app;
mod chip;
mod hex;
mod ids;
mod peripherals;
mod script;
mod jumper;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

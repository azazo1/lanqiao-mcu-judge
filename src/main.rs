mod app;
mod hex;
mod ids;
mod machine;
mod peripherals;
mod registers;
mod script;
mod timing;
mod jumper;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

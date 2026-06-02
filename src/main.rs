mod app;
mod hex;
mod ids;
mod machine;
mod peripherals;
mod registers;
mod script;
mod timing;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

mod app;
mod hex;
mod machine;
mod script;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}

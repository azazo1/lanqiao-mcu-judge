use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    stcjudge::run_cli().await
}

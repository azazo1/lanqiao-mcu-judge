use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    lanqiao_mcu_judge::run_cli().await
}

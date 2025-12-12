use prefer::load;

#[tokio::main]
async fn main() -> prefer::Result<()> {
    // Load configuration from any supported format
    // Searches for: settings.json, settings.yaml, settings.toml, etc.
    let config = load("settings").await?;

    // Access values using dot notation
    let app_name: String = config.get("app.name")?;
    let port: u16 = config.get("server.port")?;
    let debug: bool = config.get("app.debug")?;

    println!("Application: {}", app_name);
    println!("Port: {}", port);
    println!("Debug mode: {}", debug);

    Ok(())
}

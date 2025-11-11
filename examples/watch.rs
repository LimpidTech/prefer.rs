use prefer::watch;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Watching for configuration changes...");
    println!("Modify your settings file to see updates");

    let mut receiver = watch("settings").await?;

    while let Some(config) = receiver.recv().await {
        println!("\n--- Configuration updated ---");

        if let Ok(app_name) = config.get::<String>("app.name").await {
            println!("App name: {}", app_name);
        }

        if let Ok(port) = config.get::<u16>("server.port").await {
            println!("Port: {}", port);
        }

        if let Ok(debug) = config.get::<bool>("app.debug").await {
            println!("Debug: {}", debug);
        }
    }

    Ok(())
}

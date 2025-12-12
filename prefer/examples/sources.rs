//! Example showing how to use multiple configuration sources.
//!
//! Run with: cargo run --example sources

use prefer::{ConfigBuilder, ConfigValue};

// Helper to create ConfigValue objects more easily
fn obj(items: Vec<(&str, ConfigValue)>) -> ConfigValue {
    ConfigValue::Object(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn str(s: &str) -> ConfigValue {
    ConfigValue::String(s.to_string())
}

fn int(i: i64) -> ConfigValue {
    ConfigValue::Integer(i)
}

fn bool_val(b: bool) -> ConfigValue {
    ConfigValue::Bool(b)
}

#[tokio::main]
async fn main() -> prefer::Result<()> {
    // Build configuration from multiple sources
    // Later sources override earlier ones
    let config = ConfigBuilder::new()
        // Start with default values
        .add_defaults(obj(vec![
            (
                "server",
                obj(vec![("host", str("localhost")), ("port", int(8080))]),
            ),
            (
                "database",
                obj(vec![
                    ("host", str("localhost")),
                    ("port", int(5432)),
                    ("name", str("myapp")),
                ]),
            ),
            ("debug", bool_val(false)),
        ]))
        // Environment variables can override (prefix: MYAPP)
        // e.g., MYAPP__SERVER__HOST=0.0.0.0
        .add_env("MYAPP")
        .build()
        .await?;

    // Access configuration values
    let host: String = config.get("server.host")?;
    let port: u16 = config.get("server.port")?;
    let debug: bool = config.get("debug")?;

    println!("Server Configuration:");
    println!("  Host: {}", host);
    println!("  Port: {}", port);
    println!("  Debug: {}", debug);

    println!("\nDatabase Configuration:");
    let db_host: String = config.get("database.host")?;
    let db_port: u16 = config.get("database.port")?;
    let db_name: String = config.get("database.name")?;
    println!("  Host: {}", db_host);
    println!("  Port: {}", db_port);
    println!("  Name: {}", db_name);

    println!("\nTip: Set MYAPP__DEBUG=true to enable debug mode");
    println!("Tip: Set MYAPP__SERVER__PORT=3000 to change the port");

    Ok(())
}

//! Example showing how to use the derive macro for automatic type conversion.
//!
//! Run with: cargo run --example derive --features derive

#![allow(dead_code)]
use prefer::{Config, ConfigValue};
// Import the derive macro
use prefer_derive::FromValue;
// Import the trait for calling from_value()
use prefer::value::FromValue as FromValueTrait;

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

#[derive(Debug, FromValue)]
struct ServerConfig {
    host: String,
    #[prefer(default = "8080")]
    port: u16,
    #[prefer(default)]
    debug: bool,
}

#[derive(Debug, FromValue)]
struct DatabaseConfig {
    host: String,
    port: u16,
    #[prefer(rename = "database_name")]
    name: String,
    #[prefer(default = "postgres")]
    driver: String,
}

#[derive(Debug, FromValue)]
struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
}

#[derive(Debug, FromValue)]
#[prefer(tag = "type")]
enum Backend {
    #[prefer(rename = "postgresql")]
    Postgres {
        host: String,
        port: u16,
    },
    Sqlite {
        path: String,
    },
}

#[tokio::main]
async fn main() -> prefer::Result<()> {
    // Create a config from ConfigValue
    let config = Config::new(obj(vec![
        (
            "server",
            obj(vec![("host", str("localhost")), ("debug", bool_val(true))]),
        ),
        (
            "database",
            obj(vec![
                ("host", str("db.example.com")),
                ("port", int(5432)),
                ("database_name", str("myapp")),
            ]),
        ),
    ]));

    let server: ServerConfig = config.extract("server")?;
    println!("Server: {:?}", server);
    println!("  Host: {}", server.host);
    println!("  Port: {} (default was applied)", server.port);
    println!("  Debug: {}", server.debug);

    let database: DatabaseConfig = config.extract("database")?;
    println!("\nDatabase: {:?}", database);
    println!("  Host: {}", database.host);
    println!("  Port: {}", database.port);
    println!("  Name: {} (from renamed field)", database.name);
    println!("  Driver: {} (default was applied)", database.driver);

    // Example with tagged enum
    let postgres_config = Config::new(obj(vec![
        ("type", str("postgresql")),
        ("host", str("localhost")),
        ("port", int(5432)),
    ]));

    let backend: Backend = <Backend as FromValueTrait>::from_value(postgres_config.data())?;
    println!("\nBackend: {:?}", backend);

    Ok(())
}

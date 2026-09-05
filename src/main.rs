use std::{env, error::Error, io};

use tracing_subscriber::EnvFilter;
use zadt::{
    Client, FunctionGroup, FunctionModule, ObjectKey, Operation, ReqwestTransport, TransportExt,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("zadt=debug")),
        )
        .init();

    let destination = required_env("SAP_DESTINATION")?;
    let sap_client = required_env("SAP_CLIENT")?;
    let username = required_env("SAP_USERNAME")?;
    let password = required_env("SAP_PASSWORD")?;
    let language = env::var("SAP_LANGUAGE").unwrap_or_else(|_| "EN".to_owned());
    let accept_invalid_tls = enabled("SAP_DANGER_ACCEPT_INVALID_TLS");

    let transport = ReqwestTransport::builder()
        .destination(destination)
        .sap_client(sap_client)
        .language(language)
        .basic_auth(username, password)
        .danger_accept_invalid_certs(accept_invalid_tls)
        .danger_accept_invalid_hostnames(accept_invalid_tls)
        .build()?
        .traced();
    let transport = if enabled("ZADT_LOG_BODIES") {
        transport.with_body_logging(64 * 1024)
    } else {
        transport
    };

    let client = Client::new(transport).discover().await?;

    let object = ObjectKey::<FunctionGroup>::new("Z_TEST");

    let source = object
        .subobject::<FunctionModule>("Z_TEST_FUNC")
        .query()
        .execute(&client)
        .await?
        .source()?
        .query()
        .execute(&client)
        .await?;

    println!("{}", source.content);

    Ok(())
}

fn enabled(name: &str) -> bool {
    env::var(name).is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
}

fn required_env(name: &str) -> Result<String, io::Error> {
    env::var(name).map_err(|source| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing required environment variable `{name}`: {source}"),
        )
    })
}

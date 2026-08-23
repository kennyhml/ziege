use std::{env, error::Error, io};

use tracing_subscriber::EnvFilter;
use zadt::{
    AdtUri, CheckRunArtifact, CheckRunObject, Client, DataDefinition, ObjectCheckRun,
    ObjectVersion, Operation, Program, ReqwestTransport, TransportExt,
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

    let transport = ReqwestTransport::builder()
        .destination(destination)
        .sap_client(sap_client)
        .language(language)
        .basic_auth(username, password)
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?
        .traced()
        .with_body_logging(64 * 1024);

    let client = Client::new(transport).discover().await?;

    let object = client
        .object::<DataDefinition>("I_BUSINESSPARTNER")?
        .query()
        .execute(&client)
        .await?;

    let source = object.source()?.query().execute(&client).await?;

    println!("{}", source.content);

    Ok(())
}

fn required_env(name: &str) -> Result<String, io::Error> {
    env::var(name).map_err(|source| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing required environment variable `{name}`: {source}"),
        )
    })
}

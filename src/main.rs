use std::{env, error::Error, io};

use tracing_subscriber::EnvFilter;
use zadt::{
    AccessMode, AdtUri, Class, ClassSourceComponent, Client, GlobalWorkbenchType, Operation,
    Program, ReqwestTransport, TransportCheck, TransportCheckOperation, TransportCreateBuilder,
    TransportExt, TransportsQueryBuilder,
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
        .build()?
        .traced()
        .with_body_logging(64 * 1024);
    let client = Client::new(transport).discover().await?;

    let object_type: GlobalWorkbenchType = "CLAS/OC".parse()?;
    let object = client.object::<Class>("ztarftar")?;

    let session = client.create_user_session();

    let lock = object.lock(AccessMode::Modify).execute(&session).await?;
    let res = object
        .component_source(ClassSourceComponent::Macros)
        .update(&lock, "*Hi this is ZADT hheheheh!")?
        // .transport(lock.transport_request().unwrap_or_default())
        // .transport("A4HK900125")
        .execute(&session)
        .await;

    lock.remove().execute(&session).await?;
    println!("{res:#?}");

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

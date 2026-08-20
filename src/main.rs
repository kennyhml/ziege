use std::{env, error::Error, io};

use tracing_subscriber::EnvFilter;
use zadt::{
    AdtUri, CheckRunArtifact, CheckRunObject, CheckRunReportersQuery, Client, ObjectCheckRun,
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
        .build()?
        .traced()
        .with_body_logging(64 * 1024);

    let client = Client::new(transport).discover().await?;

    let object = client.object::<Program>("ZZTFTFRT")?;
    let source_uri = AdtUri::parse(&format!("{}/source/main", object.uri()))?;

    let reporters = CheckRunReportersQuery::new().execute(&client).await?;
    println!("{reporters:#?}");

    let mut run = ObjectCheckRun::new();

    run.push_object(
        CheckRunObject::new(&object, ObjectVersion::WorkingArea).artifact(CheckRunArtifact::new(
            source_uri,
            "text/plain; charset=utf-8",
            b"REPORT zztftfrt.\n\nthis is not valid abap.\n",
        )),
    );

    run.extend_reporters(reporters);

    let res = run.execute(&client).await?;

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

use std::{env, error::Error, io};

use zadt::{Class, ClassCreateProperties, Client, ObjectSnapshot, Operation, ReqwestTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = ReqwestTransport::builder()
        .destination(required_env("SAP_DESTINATION")?)
        .sap_client(required_env("SAP_CLIENT")?)
        .language(env::var("SAP_LANGUAGE").unwrap_or_else(|_| "EN".to_owned()))
        .basic_auth(required_env("SAP_USERNAME")?, required_env("SAP_PASSWORD")?)
        .danger_accept_invalid_certs(env_flag("SAP_DANGER_ACCEPT_INVALID_CERTS"))
        .danger_accept_invalid_hostnames(env_flag("SAP_DANGER_ACCEPT_INVALID_HOSTNAMES"))
        .build()?;

    let client = Client::new(transport).discover().await?;

    let mut arguments = env::args().skip(1);
    let usage = "usage: object_creation <class> <package> [transport]";
    let name = arguments
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;
    let package = arguments
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;
    let transport = arguments.next();
    if arguments.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, usage).into());
    }

    let properties = ClassCreateProperties::builder()
        .description("Hello from ZADT!")
        .package(package.as_str())
        .is_final(true)
        .build()?;

    let mut request = client.object::<Class>(&name)?.create(properties);

    if let Some(transport) = transport {
        request = request.transport(transport);
    }

    let result: Option<ObjectSnapshot<Class>> = request.execute(&client).await?;
    println!("{result:#?}");

    Ok(())
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn required_env(name: &str) -> Result<String, io::Error> {
    env::var(name).map_err(|source| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing required environment variable `{name}`: {source}"),
        )
    })
}

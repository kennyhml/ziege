use std::{env, error::Error, io};

use zadt::{Client, Operation, ReqwestTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = ReqwestTransport::builder()
        .destination(required_env("SAP_DESTINATION")?)
        .sap_client(required_env("SAP_CLIENT")?)
        .language(env::var("SAP_LANGUAGE").unwrap_or_else(|_| "EN".to_owned()))
        .basic_auth(required_env("SAP_USERNAME")?, required_env("SAP_PASSWORD")?)
        .build()?;
    let client = Client::new(transport).discover().await?;

    let username = env::args().nth(1).expect("a username is provided");

    let user_list = client.users().execute(&client).await?;
    let user = user_list
        .users
        .iter()
        .find(|u| u.as_str() == username)
        .expect("the user must exist");

    let details = user.details().execute(&client).await?;
    println!("{details:?}");

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

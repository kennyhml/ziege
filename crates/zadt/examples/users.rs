mod common;

use std::{env, error::Error};

use zadt::{Client, Operation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = common::reqwest_transport()?;
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

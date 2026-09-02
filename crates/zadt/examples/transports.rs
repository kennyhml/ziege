mod common;

use std::{env, error::Error};

use zadt::{
    Client, Operation, QueryTransportKind, TransportPropertiesQuery, TransportRequest,
    TransportsQuery,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = common::reqwest_transport()?;

    let client = Client::new(transport).discover().await?;
    if let Some(transport_number) = env::args().nth(1) {
        if let Some(transport) = TransportPropertiesQuery::new(&transport_number)
            .execute(&client)
            .await?
        {
            print_transport(&transport);
        } else {
            println!("transport `{transport_number}` was not found");
        }
        return Ok(());
    }

    let transports = TransportsQuery::builder()
        .kind(QueryTransportKind::All)
        .build()?
        .execute(&client)
        .await?;

    for request in transports.requests {
        print_transport(&request);
    }

    Ok(())
}

fn print_transport(request: &TransportRequest) {
    println!(
        "{}\t{}\t{}\t{} {}\t{}",
        request.number,
        request.kind,
        request.status,
        request.date,
        request.time,
        request.description
    );
}

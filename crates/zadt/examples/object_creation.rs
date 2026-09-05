mod common;

use std::{env, error::Error, io};

use serde_json::json;
use zadt::{Class, ClassCreateProperties, Client, ObjectKey, ObjectType, Operation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = common::reqwest_transport()?;
    let client = Client::new(transport).discover().await?;

    let mut arguments = env::args().skip(1);
    let usage = "usage: object_creation <class> <package> [typed|erased] [transport]";
    let name = arguments
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;

    let package = arguments
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;

    let mode = arguments.next().unwrap_or_else(|| "typed".to_owned());
    let transport = arguments.next();
    if arguments.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, usage).into());
    }

    match mode.as_str() {
        "typed" => {
            // build properties via statically typed payload type
            let properties = ClassCreateProperties::builder()
                .description("Hello from ZADT!")
                .package(package.as_str())
                .is_final(true)
                .build()?;

            // get a typed object key and pass the creation payload
            let mut request = ObjectKey::<Class>::new(&name).create(properties);

            // optionally add a transport
            if let Some(transport) = transport.as_deref() {
                request = request.transport(transport);
            }

            request.execute(&client).await?;
        }
        "erased" => {
            // build properties via JSON, field names are the xml qualifiers!
            let properties = json!({
                "@adtcore:description": "Hello from ZADT!",
                "@class:final": true,
                "adtcore:packageRef": {
                    "@adtcore:name": package.as_str()
                }
            });

            // get an erased object key and pass the creation payload
            let mut request = ObjectKey::from_workbench_type(&Class::WORKBENCH_TYPE, &name)?
                .create(properties)?;

            // optionally add a transport
            if let Some(transport) = transport.as_deref() {
                request = request.transport(transport);
            }

            request.execute(&client).await?;
        }
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, usage).into()),
    }

    println!("Created {name}");

    Ok(())
}

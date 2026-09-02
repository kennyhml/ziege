mod common;

use std::{env, error::Error, io};

use zadt::{Class, Client, DeletionObject, ObjectDeletion, Operation, TransportNumber};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = common::reqwest_transport()?;
    let client = Client::new(transport).discover().await?;

    let mut arguments = env::args().skip(1);
    let usage = "usage: deletion <class> [transport] [--check-only]";
    let name = arguments
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;

    let mut transport = None;
    let mut check_only = false;
    for argument in arguments {
        if argument == "--check-only" {
            if check_only {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, usage).into());
            }
            check_only = true;
        } else if transport.is_none() {
            transport = Some(TransportNumber::from(argument));
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, usage).into());
        }
    }

    // Checking deletion of an object
    let reference = client.object::<Class>(&name)?;
    let checked = reference.deletion_check().execute(&client).await?;
    println!("check: {checked:#?}");

    if !checked.objects.iter().all(|object| object.is_deletable) {
        return Err(io::Error::other("the backend rejected deletion").into());
    }

    if check_only {
        return Ok(());
    }

    // Deletion requires a transport if the object is transportable
    let object = match transport {
        Some(transport) => DeletionObject::new(&reference).transport(transport),
        None => DeletionObject::new(&reference),
    };

    let mut deletion = ObjectDeletion::new();
    deletion.push_object(object);
    let deleted = deletion.execute(&client).await?;
    println!("delete: {deleted:#?}");

    if !deleted.objects.iter().all(|object| object.is_deleted) {
        return Err(io::Error::other("the backend did not delete every object").into());
    }

    Ok(())
}

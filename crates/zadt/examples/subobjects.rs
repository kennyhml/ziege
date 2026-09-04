mod common;

use std::{env, error::Error, io};

use zadt::{Client, FunctionGroup, FunctionModule, ObjectRef, ObjectType, Operation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = common::reqwest_transport()?;
    let client = Client::new(transport).discover().await?;

    let mut arguments = env::args().skip(1);
    let usage = "usage: subobjects <function-group> <function-module> [typed|erased]";
    let group_name = arguments
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;

    let module_name = arguments
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;

    let mode = arguments.next().unwrap_or_else(|| "typed".to_owned());
    if arguments.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, usage).into());
    }

    match mode.as_str() {
        "typed" => {
            // get the typed primary object
            let group = ObjectRef::<FunctionGroup>::new(&group_name);
            // get the sub-object through the primary object with static checks
            let module = group.subobject::<FunctionModule>(&module_name);
            // the sub-object can now be treated like a regular object
            let snapshot = module.query().execute(&client).await?;
            println!("{snapshot:#?}");
        }
        "erased" => {
            // get the erased primary object
            let group =
                ObjectRef::from_workbench_type(&FunctionGroup::WORKBENCH_TYPE, &group_name)?;

            // get the erased sub-object, no static guarantees can be made, this is a runtime check
            let module = group.subobject(&FunctionModule::WORKBENCH_TYPE, &module_name)?;

            // the sub-object can now be treated like a regular erased object
            let snapshot = module.query().execute(&client).await?;
            println!("{snapshot:#?}");
        }
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, usage).into()),
    }

    Ok(())
}

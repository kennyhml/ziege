# zadt

ABAP Development Tools protocol for the Ziege framework.

## Background

ABAP Development Tools are more than an Eclipse plugin. The application server
exposes an HTTP API for working with repository objects stored on the system.
ZADT provides typed operations and runtime dispatch for that API.

## Getting started

### Basics

Create a typed reference from the collection advertised by discovery:

```rust,ignore
let class = client.object::<Class>("ZCL_DEMO")?;
```

Operations that only need object identity can use the reference directly:

```rust,ignore
let activation_result = class.activation().execute(&client).await?;
```

Source navigation requires loaded properties because ADT advertises source URIs in
the object representation:

```rust,ignore
let class = class.query().execute(&client).await?;
let source = class.source()?.query().execute(&client).await?;
```

`Class` is an alias for `AdtObject<ClassProperties>`. Its static capabilities come
from traits such as `Source`. Its loaded properties determine which concrete source
resources are available. This lets ZAFF project modern and legacy class layouts
without guessing paths.

### Runtime descriptors

Runtime object types use `ObjectRef<()>`. A modeled runtime reference can load an
`AdtObject<serde_json::Value>`. ZADT selects a registered descriptor from the exact
Workbench type and validates capabilities at runtime.

Typed operations dispatch through Rust traits. Runtime operations dispatch through
descriptors and return explicit errors when an object type does not support an operation.

### Transport

ZADT uses a transport trait around ADT requests and responses. Custom transports are
supported. The provided implementation uses HTTP through `reqwest`.

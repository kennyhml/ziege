# zadt

ABAP Development Tools protocol for ziege.

## Background

ABAP Development Tools are more than an Eclipse plugin. The application server
exposes it as an API for working with repository objects stored on the system.
ZADT provides typed operations and runtime dispatch for that API.

## Getting started

### Basics

Create a typed reference from the collection advertised by discovery:

```rust,ignore
let class = client.object::<Class>("ZCL_DEMO")?;
```

Only primary object types can be constructed directly from a discovery
collection. Subobjects are constructed through a parent reference and the URI
templates advertised by that parents collection:

```rust,ignore
let group = client.object::<FunctionGroup>("Z_GROUP")?;
let module = group.subobject::<FunctionModule>("Z_FUNCTION")?;
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

ADT identities are represented as resource handles, such as `SourceRef`. Handles expose operations
naturally associated with their context, while operation values remain independently composable 
and executable. The API also makes a clean split between creating operations and executing them,
which allows a large portion of the API to remain sync.

### Shapshots
Because of the remote nature of the development system, a fetched representation of some development
object locally can make no guarantees that it is the most up-to-date version on the system. That would
require all fetched objects to be locked at all times. Because of this, an `ObjectRef<T>.query()` returns
an `ObjectSnapshot<T>` which includes the etag, workbench version and properties. It is merely a snapshot
of the object at the time of querying it - not a domain model! Some operations can only dispatch through
the snapshot because they require the advertised relations of the object to resolve. For instance, not all
global classes have a testclasses include so simply constructing a `/sap/bc/adt/oo/classes/zcl_demo/source/testclasses`
operation is flawed because we make an assumption that the resource **has** the include and that its at that
specific location - if the location changed, our code would break.

As you may have noticed, this is a trade-off between efficiency and correctness. Making an assumption about 
the location could be correct almost all of the time and avoid an extra round-trip. But it is a case where
'look before you leap' is better suited than 'easier to ask for forgiveness than permission' in python terms.
It sets the API up to be more compatible and uses HATEOAS as it was intended.

### Descriptors
The API has two fundamental ways of using it. Through a typed object reference `ObjectRef<T>`, the compiler
knows what operatinos are valid for a reference such as `ObjectRef<Class>` or `ObjectSnapshot<Class>` for that
matter. It also knows associated properties and sub-objects and adds many compile time guarantees.

To handle objects more dynamically, for example when a workbench type and name is provided in a code editor
or a command line interface, a seperate, descriptor backed type erased path exists, namely `ObjectRef<()>` and
`ObjectSnapshot<()>`. These object references can make no static guarantees about their operations. Operations
that are not always valid (such as updating or creating an object) are turned into results with an unsupported
error that can only be caught at runtime. Internally, they all still use the underlying static capabilities
erased through a common set of type monomorphized function pointers - similar to a vtable.

### Transport

ZADT uses a transport trait around ADT requests and responses. Custom transports are
supported. The provided implementation uses HTTP through `reqwest`.

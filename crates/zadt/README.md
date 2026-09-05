# zadt

ABAP Development Tools protocol for ziege.

## Background

ABAP Development Tools are more than an Eclipse plugin. The application server
exposes it as an API for working with repository objects stored on the system.
ZADT provides typed operations and runtime dispatch for that API.

## Getting started

### Basics

Object identity, location, and loaded state are separate:

- `ObjectKey<T>` is a logical identity: normalized name, Workbench type, and any
  known logical parent. It has no URI. Discovery supplies the location when needed.
- `ObjectRef<T>` is a located identity: a key, a mandatory validated `AdtUri`, and
  optional immediate parent URI metadata. It does not contain loaded properties.
- `ObjectSnapshot<T>` is a loaded representation: a located reference, properties,
  Workbench version, media type, and optional ETag.

Both a logical key and a located reference construct the same operation type:

```rust
use zadt::{AdtUri, Class, ObjectKey, ObjectQuery, ObjectRef};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let key = ObjectKey::<Class>::new("zcl_demo"); // Normalized to ZCL_DEMO.
let located = ObjectRef::new(
    key.clone(),
    AdtUri::parse("/sap/bc/adt/oo/classes/zcl_demo")?,
);

let logical_query: ObjectQuery<Class> = key.query();
let located_query: ObjectQuery<Class> = located.query();
# Ok(())
# }
```

The logical query resolves its target through discovery. The located query uses
the retained URI, even if discovery would derive a different location for that
key. `AdtUri` validates the resource path; constructing a reference does not verify
that the object exists at that location.

`ObjectQuery<T>` conservatively declares `RequiresDiscovery` for both entry
points. This is a property of the operation type, not its current target value, so
even a located query executes through a discovery-enabled client. Keeping a URI
does not make every operation discovery-independent: other operations can still
need advertised service endpoints, templates, media types, or parent context.

Discovery can also resolve a key explicitly:

```rust,ignore
let located = client.discovery().resolve_object(&key)?;
let uri = client.discovery().resolve_object_uri(&key)?;
```

`resolve_object` returns an `ObjectRef<T>` with its own and any immediate logical
parent's URI resolved. `resolve_object_uri` returns only the object's URI and
always uses logical discovery-based addressing.

Located references retain their own URI and parent metadata when cloned, erased,
or recovered as a typed reference. `located.key()` explicitly selects logical
identity; querying that key again opts into discovery-based addressing instead of
using the retained location. There is no implicit dereference from a reference to
its key.

Operations that do not need loaded properties can use a key or located reference:

```rust,ignore
let activation_result = located.activation().execute(&client).await?;
```

Source navigation requires loaded properties because ADT advertises source URIs
in the object representation:

```rust,ignore
let snapshot = located.query().execute(&client).await?;
let source = snapshot.source()?.query().execute(&client).await?;
```

Related-resource handles such as `SourceRef` retain their owning located object
and expose operations appropriate to their context. Constructing operations is
synchronous; executing them performs the remote work.

### Subobjects and Creation

Only primary object keys can be constructed directly with `ObjectKey::new`.
Logical subobjects are constructed through a parent key, not a located reference:

```rust
use zadt::{FunctionGroup, FunctionModule, FunctionModuleCreateProperties, ObjectKey};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let group = ObjectKey::<FunctionGroup>::new("Z_GROUP");
let module = group.subobject::<FunctionModule>("Z_FUNCTION");

let creation = module.create(
    FunctionModuleCreateProperties::builder()
        .description("Demo function module")
        .build()?,
);
# Ok(())
# }
```

Creation is key-only, for both primary objects and subobjects. It discovers the
collection to which the creation payload is posted; it is not a write to an
already located object's URI. The child's key supplies the logical parent needed
to resolve the relationship and populate the creation payload. `ObjectRef` does
not expose `subobject` or `create`.

A located child received from ADT need not include a logical parent key or parent
URI. Missing parent metadata does not prevent operations such as querying the
child at its known URI. Operations whose protocol requires parent context, such
as child activation, use the retained `parent_uri` first, otherwise resolve the
logical `key().parent()`, and fail if the required context is unavailable. Parent
context is not resolved just to use a located object's own URI.

`with_parent_uri` attaches a known immediate parent URI without constructing a
recursive resolved parent reference. Key deserialization permits parentless
children for advertised inputs, but logical resolution still fails when the
object type requires a parent and none is known.

Key equality includes the logical parent. Located reference equality and hashing
compare name, Workbench type, and URI, ignoring both logical parent metadata and
the optional parent URI.

### Snapshots

Queries from either keys or located references return `ObjectSnapshot<T>`. A
snapshot describes the object at the time of the response; it does not guarantee
that the server state remains unchanged. Its `reference()` returns the located
`ObjectRef<T>`, and `uri()` delegates to that reference. Cloning a snapshot or
converting between typed and erased snapshots preserves the location and known
parent metadata. Follow-up queries and property updates retain that location
rather than deriving it again from the logical key.

Some operations require the relations advertised by loaded properties. For
example, not every global class has a test-classes include, and an existing
include need not live at an assumed path. Source navigation uses advertised
relations instead of synthesizing such locations. This can require an extra
round trip, but avoids assuming either a capability or its resource path.

### Descriptors

Typed keys, references, and snapshots, such as `ObjectKey<Class>`,
`ObjectRef<Class>`, and `ObjectSnapshot<Class>`, let the compiler select supported
operations and associated property types. Typed parent keys also restrict which
subobjects can be constructed.

The runtime forms, `ObjectKey<()>`, `ObjectRef<()>`, and `ObjectSnapshot<()>`,
retain the Workbench type as data. They are useful when object identity comes from
a code editor, command line, or repository response. Capabilities that vary by
object family are checked at runtime and can return an unsupported-type or
unsupported-capability error. Internal descriptors dispatch to the same concrete
property models and capability implementations through monomorphized function
pointers. Erasing or recovering a reference's type does not discard its location.

### Wire Model Strictness

Complete deserialized models reject unknown fields, including nested references,
links, response wrappers, and generated creation payloads. The same policy applies
to XML and wire-shaped runtime JSON so unmodeled fields are not silently dropped
when editing and serializing properties. New backend fields require an explicit
model update; intentionally open scalar vocabularies such as `Other(String)` remain
open.

Keys and located references also use strict Serde models. A key contains `name`,
`object_type`, and an optional logical `parent`. A located reference contains
`key`, mandatory `uri`, and optional `parent_uri`; it cannot deserialize from a
bare key. Names normalize to ASCII uppercase, typed keys validate their Workbench
type, supplied logical parents must have a supported relationship, and both URI
fields pass through `AdtUri` validation.

This is a Serde field policy, not full XML schema validation: root names, namespace
URIs, and attributes on scalar-valued XML elements are not validated by this policy.
The internal version-only projection runs only after strict full-property decoding.
Transport request attributes (`REQ_ATTRS`) currently have only an evidenced empty
representation; populated contents fail closed until their schema is modeled.

### Transport

ZADT uses a transport trait around ADT requests and responses. Custom transports are
supported. The provided implementation uses HTTP through `reqwest`.

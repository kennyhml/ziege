# zadt

ABAP Development Tools protocol for the Ziege framework.

## Background

The ABAP Development Tools are more than just an Eclipse plugin. On the actual ABAP 
Application Server, they represent a whole framework for ABAP development tooling 
exposed via RFC or ICF (HTTP). The idea is similar to how the more modern language server
protocol works. Although while a language server intentionally decouples sender from receiver, 
ABAP development generally happens on the system itself and **must** serve language server like
features via external communication.

For anyone curious about them, you can enable the `ABAP Communication Log` view inside ecplise.

## Design

### Discovery

`ZADT` attempts to make proper use of the HATEOAS design of the ADT API. Unfortunately, this aspect is
almost always ignored by clients using such APIs, which makes it difficult for the provider of the API
to make changes that would otherwise be non breaking, such as changing the location of a resource.

Nevertheless, `ZADT` still uses if on the off-chance it ever happens, which also improves compatibility
going forward (and backwards). In simpler terms, this means when you create an object reference such as
```rust,ignore
let program = client.object::<Program>("ZDEMO")?;
```
The client checks the discovery collection for the stable `Program` schema and from there, determines 
that its location is advertised as `/sap/bc/adt/programs/programs`. Another advantage is that supported
media-types can be negotiated and chosen dynamically while remaining fully typed.

Its worth noting that HATEOAS doesnt mean the client does not know about the semantics of resources.
Operations and responsens remain fully typed. It mainly lets the provider change locations and 
syntactic details of a resource, such as the location (not the name!) of a uri parameter for a 
template action, without breaking clients relying on it.

### Typing

`ZADT` tries to provide full typing support and uses typestate pattern in various places to introduce
invariants. For example, the current typing system makes it impossible to call a stateful operation
without a stateful session context. It also provides convenient methods to navigate between objects
and their relations. You can find more on the technical details down below.

ADT object types often arrive at runtime from a command line or network request. `ZADT` supports that
case with a descriptor-backed `ObjectRef<Erased>` while retaining `ObjectRef<T>` for statically known
object types. Each modeled type has one `#[object_type(...)]` declaration that generates its static trait
implementations and a private runtime descriptor. The explicit registry lists only those generated
descriptors, so discovery identity, source capabilities, and property parsing come from the
same declaration. RIS entries with an unmodeled type retain their identity without pretending that
family-specific operations exist.

`GlobalWorkbenchType` preserves the exact ADT registry identifier. Although values commonly look like
`CLAS/OC`, the vocabulary is opaque and also includes compact values such as `AUTH` and identifiers with
more than one slash; callers should compare the complete value rather than decomposing it.

Common runtime operations dispatch through that descriptor. Typed references expose their concrete
property models, while `ObjectRef<Erased>` exposes the same representations as validated JSON together
with their originating object, media type, and ETag. Runtime property updates deserialize through the registered concrete
model before creating XML, so unsupported object types and read-only property models fail explicitly.

Programs and classes retain distinct typed run operations because ADT advertises
separate protocol contracts for them. Runtime callers can use the same capability
through `ObjectRef<Erased>::run()` without trying each modeled object type:

```rust,ignore
let typed_output = program.run().execute(&client).await?;
let runtime_output = repository_object.run()?.execute(&client).await?;
```

Primary source and secondary component sources are modeled separately. `ObjectRef<T>::source()` and
`ObjectRef<Erased>::source()` resolve the primary source, while class definitions, implementations, and
other includes are exposed through `component_source(...)` and `source_component(...)`. Runtime component
enumeration therefore never includes a synthetic `main` component.

Source updates remain resource-driven rather than descriptor-dispatched. A `SourceRef` retains its owning
object and accepts any modification lock for that object. The lock remains independent from an individual
source component, allowing one class lock to update definitions, implementations, and other includes:

```rust,ignore
let session = client.create_user_session();
let lock = class.lock(AccessMode::Modify).execute(&session).await?;

let result = class
    .component_source(ClassSourceComponent::Definitions)
    .update(&lock, definitions)?
    .execute(&session)
    .await?;

class.unlock(lock)?.execute(&session).await?;
```

When SAP attaches a transport request to the lock, `SourceRef::update` carries
it into the update automatically. When the first lock does not have a request
yet, set the selected request explicitly with `.transport(&request)`.
Transport identifiers are exposed as lossless `TransportNumber` values rather
than interchangeable strings.

### Transport

The transport layer can be conveniently abstracted away as, regardless of the protocol used, ADT wraps
the request and response in a HTTP-like structure anyways. RFC only ends up being a tunnel to transport
that data to and from the server. For this reason, `ZADT` supports custom, protocol agnostic transport
implementations. Currently only, HTTP transport using `reqwest` is implemented out of the box.


### Techical details

This is more of a flex about how awesome rust is and, unless you plan to contribute, can probably be
ignored.

The most important trait in `ZADT` is the `Operation` trait, defines as

```rust,ignore
pub trait Operation<S: ClientState>: Send + Sync {
    type Response: Send;
    type Kind: OperationKind;

    fn request(&self, client: &Client<S>) -> Result<AdtRequest, OperationError>;

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError>;
}
```

An operation is effectively a request which its implementor can construct in the `request` method.
It is generic over the client state `S`, which defines whether the client has performed discovery
and can provide operations with a collection, and the operation kind `OperationKind` which defines
whether an operation is `Stateful` or `Statelss`.

With that in place, an execution trait can be defined as
```rust,ignore
pub trait Execute<S, O>: Send + Sync
where
    S: ClientState,
    O: Operation<S>,
{
    fn execute(&self, operation: &O) -> impl Future<Output = Result<O::Response, OperationError>> + Send;
}
```
which means that a `Client` with state `S` can execute operations for state `S`.. This provides a compile 
time invariant for operations that rely on the discovery data to dispatch. Of course, entry point operations 
such as the discovery itself, are valid for any client state.

The associated type `Kind` enforces the next invariant, which is calling stateful operations outside
of a stateful context. To implement that, a user session wraps a client
```rust,ignore
pub struct UserSession<S: ClientState> {
    client: Client<S>,
    state: Mutex<UserSessionState>,
}
```
And then we implement the stateful execution only for `UserSession<S>`:
```rust,ignore
impl<S, O> Executo<S, O> for UserSession<S>
where
    S: ClientState,
    O: Operation<S, Kind = Stateful>,
{
    async fn execute(&self, operation: &O) -> Result<O::Response, OperationError> {
        ...
    }
}
```
The best part about this design is that it allows for some sweet decorator patterns. For instance,
etag handling can simply be implemented like this:
```rust,ignore
pub enum Revalidation<T> {
    Modified(T),
    NotModified { etag: Option<EntityTag> },
}

pub struct IfNoneMatch<O> {
    inner: O,
    etag: EntityTag,
}

impl<S, O> Operation<S> for IfNoneMatch<O>
where
    S: ClientState,
    O: Operation<S>,
{
    type Response = Revalidation<O::Response>;
    type Kind = O::Kind;

    fn request(&self, client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = self.inner.request(client)?;
        request.set_cache_revalidation(Some(&self.etag));
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(Revalidation::NotModified {
                etag: response.entity_tag(),
            });
        }

        self.inner.decode(response).map(Revalidation::Modified)
    }
}
```
This can also drive batching, paging, retry behavior and much more.

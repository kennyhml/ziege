# zaff

ABAP File Formats (AFF) specifications and projection of loaded ADT objects
for editor files and property schemas.

See [abap-file-formats](https://github.com/SAP/abap-file-formats) to understand the background and benefits
of using the ABAP file formats.

## Projection

The crate only has one relevant entry point, the `zaff::project()` method.

It takes ownership of an object snapshot - i. e. the loaded presentation of an object
at some point in time, and returns a `Projection`. The projection exposes a set
of `FileProjection` which map to some component of the object vie a `FileBacking`,
such as `SourceRef` for source components or a property codec to convert between
ADT and AFF for the general property mappings - usually the backings for the `.json` files.

This also reveals one of the drawbacks of the AFF projection. A loaded object (snapshot)
is required to project the correct files. Not only because the file bindings must bind
to data accessed through the object properties, but also because certain objects, such
as classes, do not always project to the same set of files. Depending on the age of the
class, it may have a different set of includes advertised in its properties.

ZAFF guarantees that no I/O takes place during projection. It is entirely pure and statless
because it requires all the prerequisites to be passed at projection-time.

## Editing

ZAFF is not concerned with storing or writing to source code or properties directly.

It only provides the interface to find out what resource backs a projected file and,
in the case of object properties, it provides methods to map between the AFF schema
and the ADT properties formats.

Projections are immutable. Keep the backing that produced an edited document
after saving, project the returned or refetched snapshot. ZAFF does not manage
dirty buffers, locks, conflicts, or cache refreshes.

### Example
When opening a projected file in an editor:
```rust
use zadt::{Client, Discovery, Operation};
use zaff::{FileBacking, FileProjection};

async fn read_file(
    client: &Client<Discovery>,
    file: &FileProjection,
) -> Result<String, Box<dyn std::error::Error>> {
    match file.backing() {
        FileBacking::Source(source) => {
            let loaded = source.query().execute(client).await?;
            Ok(loaded.content)
        }
        FileBacking::Properties(properties) => {
            properties.render().map_err(Into::into)
        }
    }
}
```
When writing, using optimistic locking for simplification:
```rust
use zadt::{Client, Discovery, Operation, PreconditionResult};
use zaff::{FileBacking, FileProjection};

async fn write_file(
    client: &Client<Discovery>,
    file: &FileProjection,
    contents: String,
) -> Result<(), Box<dyn std::error::Error>> {
    match file.backing() {
        FileBacking::Source(source) => {
            let result = source.update_if_match(contents)?.execute(client).await?;
            match result {
                PreconditionResult::Success(_) => {}
                PreconditionResult::Failed { .. } => {
                    return Err("source changed since the file was opened".into());
                }
            }
        }
        FileBacking::Properties(properties) => {
            if let Some(payload) = properties.merge(&contents)? {
                let result = properties.subject()
                    .update_if_match(payload)?
                    .execute(client)
                    .await?;

                match result {
                    PreconditionResult::Success(_) => {}
                    PreconditionResult::Failed { .. } => {
                        return Err("object properties changed since the file was opened".into());
                    }
                }
            }
        }
    }
    Ok(())
}
```

## Supported Formats

Source files are projected when advertised by the loaded object. Language-dependent
text files and other file specifications without an implemented mapping are omitted.
Unsupported metadata edits are rejected.

| Format | Implementation notes |
| --- | --- |
| [CLAS — Class](src/formats/clas.rs) | Component descriptions have no implemented backing. |
| [INTF — Interface](src/formats/intf.rs) | Category, proxy status, and component descriptions accept only default or empty values. |
| [PROG — Program / standalone Include](src/formats/prog.rs) | Standalone includes use PROG rather than the function-group REPS format. |
| [DTEL — Data Element](src/formats/dtel.rs) | Maps type definitions, labels, and search-help settings. |
| [DOMA — Domain](src/formats/doma.rs) | Fixed-value append names are unavailable. Optional documentation is declared but unsupported. |
| [DEVC — Package](src/formats/devc.rs) | Switch assignments have no implemented backing. |
| [DDLS — CDS Data Definition](src/formats/ddls.rs) | `sourceType` currently unsupported. |
| [DDLX — CDS Metadata Extension](src/formats/ddlx.rs) | Metadata maps the header. |
| [DDLA — CDS Annotation Definition](src/formats/ddla.rs) | Header has no ABAP language version. |
| [DCLS — CDS Access Control](src/formats/dcls.rs) | Metadata maps the header. |
| [SRVD — Service Definition](src/formats/srvd.rs) | Maps the header, origin, and definition or extension source type. |
| [FUGR — Function Group](src/formats/fugr.rs) | Children are projected separately. Group and main-program metadata share the description field. |
| [REPS — Function Group Include](src/formats/fugr.rs) | Requires the parent group name. Child discovery and folder assembly belong to the caller. |
| [FUNC — Function Module](src/formats/fugr.rs) | `includeNumber` is temporarily fixed to `"00"`. Source is passed through without AFF pseudo-syntax conversion. |



# zaff

ABAP File Formats (AFF) projection of loaded ADT objects. Object in, file
projections out. ZAFF performs no network or filesystem I/O.

## Projection

`project(snapshot)` is the only entry point. It takes ownership of an
`ObjectSnapshot<()>` and returns a `Projection`. Typed snapshots can be passed
using `snapshot.into_erased()`.

- `subject()` returns the original loaded ADT object.
- `files()` lists available file projections; `file(name)` looks one up.
- `format()` identifies the AFF family and version.

Each file has a filename, a specification, and a backing:

| Backing | Content |
| --- | --- |
| `Source(SourceRef)` | Source text fetched through ZADT using the advertised reference. |
| `Properties(PropertiesProjection)` | AFF JSON rendered from the retained ADT properties. |

```rust,no_run
# use zadt::{Client, Discovery, ObjectSnapshot, Operation};
# async fn example(snapshot: ObjectSnapshot<()>, client: &Client<Discovery>, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
use zaff::{FileBacking, project};

let projection = project(snapshot)?;
let file = projection.file(filename).ok_or("file is not available")?;

let content = match file.backing() {
    FileBacking::Properties(properties) => properties.render()?,
    FileBacking::Source(source) => source.query().execute(client).await?.content,
};
# let _ = content;
# Ok(())
# }
```

## Editing

For metadata, `properties.merge(edited)` validates AFF JSON and applies its
changes to the original ADT properties, preserving unrelated modeled fields.
It returns `None` for a validated no-op, or `Some` **ADT wire-shaped JSON** for
`properties.subject().update_if_match(...)` or `update_with_lock(...)`.
Only submit an update for `Some`; `None` compares against the retained snapshot,
not current backend state.
Source files use ZADT's lock-based source-update API instead.

Field constraints use Garde; `ProjectionError::Validation` retains its structured
report with Rust field names. ADT mapping restrictions are checked separately.

Projections are immutable. Keep the backing that produced an edited document;
after saving, project the returned or refetched snapshot. ZAFF does not manage
dirty buffers, locks, conflicts, or cache refreshes.

## Supported Families

| Family | Files |
| --- | --- |
| Class | `.clas.json`, main source, and advertised includes |
| Program / standalone Include | `.prog.json` and main source |
| Data Element | `.dtel.json` |

Each module in `src/formats/` declares one static `ObjectFormat` containing its
Workbench types and file mappings, alongside its AFF models and validation.
The registry enumerates these formats; projections hold a reference to one.

Missing sources are omitted; invalid advertised locations are errors.
Language-dependent `.properties` files are recognized specifications but are
not implemented. Unsupported AFF edits are rejected rather than silently lost.

## Integration

ZVFS lists repository objects. A language server loads their properties through
ZADT, projects them with ZAFF, and associates editor documents with the resulting
files. The server owns document state and save orchestration; AFF files do not
become ZVFS repository nodes.

Look up editor files within their retained projection using `projection.file(name)`.
Keep the connection, authoritative object reference, and version in the caller's
index; ZAFF does not infer remote objects from filenames or AFF metadata.
Source reads do not automatically inherit the properties snapshot's
version, and source validators are distinct from properties validators.

See `tests/editor_flow.rs` for the complete list, read, edit, and guarded-save
workflow using ZVFS and ZADT.

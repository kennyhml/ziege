# zaff

Bidirectional ABAP File Formats projection.

`zaff` maps repository objects from `zadt`/`zvfs` to editor-facing files and
maps those files back to their logical ADT components. It owns the transformations
between ADT models and AFF schemas, including merging an edited AFF document
back into the original ADT properties so fields outside the AFF schema are
preserved.

## Projection 

The following example shows `zaff` used through LSP orchestration. The language
server coordinates repository traversal, AFF projection, and ADT operations.


```mermaid
sequenceDiagram
    participant IDE
    participant LSP as ABAP Language Server
    participant VFS as zvfs
    participant AFF as zaff
    participant ADT as zadt
    participant SAP as SAP ADT API

    IDE->>LSP: readDirectory(project, system, parent)
    LSP->>VFS: children(parent)
    VFS->>ADT: Repository query
    ADT->>SAP: HTTP request
    SAP-->>ADT: Repository entries
    ADT-->>VFS: Typed entries
    VFS-->>LSP: Repository nodes

    LSP->>AFF: Project object to AFF files
    AFF-->>LSP: Canonical filenames/components
    LSP-->>IDE: Filesystem entries

    IDE->>LSP: readFile(resourceId)
    LSP->>AFF: Resolve AFF filename
    AFF-->>LSP: ADT source reference
    LSP->>ADT: Source query
    ADT->>SAP: GET source
    SAP-->>ADT: Source and ETag
    ADT-->>LSP: SourceCode
    LSP-->>IDE: Document content
```

Data Elements are properties-backed rather than source-backed. A `DTEL/DE`
object projects to one `<name>.dtel.json` file. The consumer executes the
typed `ObjectRef<DataElement>::query()` operation, asks `zaff` to render the
returned `DataElementProperties` as AFF JSON, and retains those original
properties. On save, `zaff` merges the edited AFF fields into that typed value
before the consumer builds and executes `ObjectRef<DataElement>::update(...)`.
This preserves ADT fields that the AFF schema does not represent.

# Limitations

Path resolution identifies an AFF family and component, not a globally unique
remote object. A consumer should retain the `RepositoryObject` used
to project every concrete path. That index disambiguates representations such
as `PROG/P` and `PROG/I`, which share the same `.prog.*` file layout, and keeps
the SAP system, package, URI, and object version attached to edits.

Optional class includes and language-dependent property files are exposed as
possible `FileSpec`s. The projection consumer decides which concrete files to
publish based on resources available from the backend. `project` returns files
that currently have a content backing; unsupported Program and Class metadata
and text codecs remain available only as specifications.

# Challenges
Legacy CLAS objects have no testclasses, macros, implementations and definitions include.
Instead, they have a `localtypes` include that modern classes do not have. This implies
that it isnt possible to correctly expand the classes without fetching the object properties
before that, which incurs I/O that technically does not belong into the projection layer.

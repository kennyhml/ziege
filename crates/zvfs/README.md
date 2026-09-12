# zvfs

A lazy virtual repository tree backed by the ADT Repository Information System.

## Background

SAP systems store repository objects in database tables rather than as local
text files. Opening an object therefore requires a connection to the SAP system
and an understanding of the remote repository protocol. The repository is not
simply a remote filesystem.

The ABAP Development Tools (ADT) expose the Repository Information System (RIS)
and its virtual-folders API for discovering repository objects. `zvfs` uses
[zadt](../zadt) for that communication and provides a higher-level API for
traversing and refreshing the resulting repository tree.

## Design

RIS structures repository objects through facets. A facet is a metadata
dimension such as package, owner, broad repository group, or concrete object
type. A request filters objects with facet preselections and names an output
facet by which RIS should group the matching objects.

For example, this request asks for packages containing classes owned by `DEVELOPER`:
```xml
<vfs:preselection facet="OWNER">
    <vfs:value>DEVELOPER</vfs:value>
</vfs:preselection>
<vfs:preselection facet="TYPE">
    <vfs:value>CLAS</vfs:value>
</vfs:preselection>
<vfs:facetorder>
    <vfs:facet>PACKAGE</vfs:facet>
</vfs:facetorder>
```

`zvfs` turns these requests and responses into caller-defined `Mount` points. A
mount is a static entry point into a facet chain that eventually leads to
repository objects. Eclipse with ADT commonly presents entries like:

```text
A4H
├── Local Objects ($TMP)
├── Favorite Packages
├── Favorite Objects
└── System Library
```

A local-objects mount can be constructed like this:

```rust,no_run
use zadt::{Client, Discovery, RepositoryFacet, RepositoryPreselection};
use zvfs::{FacetLevel, FacetPolicy, Mount, VfsError, VirtualRepositoryTree};

async fn local_objects_tree(client: Client<Discovery>) -> Result<VirtualRepositoryTree, VfsError> {
    let preselections = [
        RepositoryPreselection::directly_assigned("$TMP"),
        RepositoryPreselection::new(RepositoryFacet::OWNER, "DEVELOPER"),
    ];

    VirtualRepositoryTree::builder(client)
        .mount(
            Mount::selection("Local Objects ($TMP)", preselections).facet_policy(
                FacetPolicy::new([
                    FacetLevel::always(RepositoryFacet::OWNER),
                    FacetLevel::always(RepositoryFacet::GROUP),
                    FacetLevel::always(RepositoryFacet::TYPE),
                ]),
            ),
        )
        .build()
        .await
}
```

To include another owner's local objects, add that owner to the same preselection:

```rust
use zadt::{RepositoryFacet, RepositoryPreselection};

let preselections = [
    RepositoryPreselection::directly_assigned("$TMP"),
    RepositoryPreselection::new(RepositoryFacet::OWNER, "DEVELOPER").include("JONDOE"),
];
```
The resulting tree could look like:

```text
A4H
└── Local Objects ($TMP)
    ├── DEVELOPER
    │   ├── Dictionary
    │   │   └── Database Tables
    │   │       └── ZMYTAB
    │   └── BSP Library
    └── JONDOE
        ├── Dictionary
        └── Source Code Library
            └── Classes
                └── ZCL_MYCLASS
```

Each mount has an independent selection and facet policy. Applications can
therefore reproduce the standard ADT views or define their own, such as
mounting selected packages directly at the tree root.

## Adaptive Facet Layers

Facet layers make large selections manageable. A package can contain hundreds
or thousands of directly assigned objects, where grouping entries under folders
such as `Source Code Library/Classes` is useful. For a small selection, however,
an extra folder level may only add navigation overhead.

An adaptive facet level is retained only when the current selection contains at
least a configured number of objects. For example:

```rust
use zadt::RepositoryFacet;
use zvfs::{FacetLevel, FacetPolicy};

let policy = FacetPolicy::new([
    FacetLevel::always(RepositoryFacet::OWNER),
    FacetLevel::always(RepositoryFacet::GROUP),
    FacetLevel::adaptive(RepositoryFacet::TYPE, 30),
]);
```

Here the `TYPE` layer, which produces folders such as `Classes`, `Programs`, or
`Database Tables`, is omitted when a group contains fewer than 30 objects. The
tree can then become:

```text
A4H
└── Local Objects ($TMP)
    ├── DEVELOPER
    │   ├── Dictionary                 (TYPE layer omitted)
    │   │   └── ZMYTAB
    │   └── BSP Library
    └── JONDOE
        ├── Dictionary
        └── Source Code Library        (TYPE layer omitted)
            └── ZCL_MYCLASS
```

Adaptive decisions are evaluated independently at each configured level and
are reevaluated when a node is refreshed.

## Repository object children

Repository objects retain their object identity and can have children when SAP
advertises them as expandable. Below those objects, ZVFS uses the ADT repository
node-structure API. Its type folders are always retained, independently of the
RIS facet policy:

```text
ZGROUP123
├── Function Group Includes
│   ├── LZGROUP123TOP
│   └── LZGROUP123UXX
├── Function Modules
│   ├── ZFTFTR
│   └── ZTFATFART
└── Textelements
```

`Node::is_directory()` reflects expandability rather than excluding object nodes.
`tree.object_ref(id)` returns a retained ZADT object reference when the node has a
plain object-resource location. Source members retain their query and fragment
navigation information instead. Package metadata can be absent for related objects
whose package was not returned by the backend.

## Example: Terminal system explorer

The crate includes a Ratatui explorer with a tree widget and a selected-object details pane.
It reads connection details from `.env` or the process environment:

```text
SAP_DESTINATION=https://example.test
SAP_CLIENT=001
SAP_USERNAME=DEVELOPER
SAP_PASSWORD=secret
SAP_LANGUAGE=EN
```

Without an argument, the explorer mounts the System Library. Pass a package
name to start with a narrower package mount:

```bash
cargo run -p zvfs --example explorer
cargo run -p zvfs --example explorer -- /DMO/FLIGHT_REUSE
cargo run -p zvfs --example explorer -- '$TMP'
```

Folders load asynchronously when expanded, so navigation and quitting stay responsive
while SAP requests are running. Keyboard controls:

```text
Up/Down or k/j     move selection
Right or l         expand
Left or h          collapse or select parent
Enter / Space      toggle expansion
Home / End         first / last expanded-tree entry
Page Up / Down     move one page
/                  find a name in the expanded tree, including off-screen entries
Enter / Escape     finish finding
n                  next match
r                  refresh selected folder, or retry a failed load
p                  preload one layer below the selected folder
q / Ctrl+C         quit
```
Search only visits loaded, expanded branches. Refreshing a backend tree may remove
the selected node, in which case selection returns to its nearest surviving ancestor.
The terminal is restored when the explorer exits. For a local system with development certificates, the
example also honors `SAP_DANGER_ACCEPT_INVALID_CERTS` and
`SAP_DANGER_ACCEPT_INVALID_HOSTNAMES` from the environment.

## Technical Details

Building a tree performs one RIS facet-catalog request. The builder validates
that every configured policy facet is advertised for structuring and retains
the catalog for hierarchy-aware refresh decisions.

The tree loads each directory lazily. The graph lock is held only while reading
or mutating in-memory records and is never held across an ADT request. A
node-local asynchronous lock deduplicates concurrent first loads and coalesces
overlapping explicit refreshes of the same node without preventing separate
branches from loading concurrently.

Each node receives a tree-scoped `NodeId` containing a UUID and a monotonic
numeric index. Live records are stored in a `HashMap` keyed by that index.
Removed IDs remain stale and cannot resolve to newly inserted nodes.
Repository package and object locations are exposed as validated `AdtUri`
values rather than unchecked strings.

Refreshes reconcile one immediate layer by semantic identity: package and
object nodes use their ADT navigation location including query and fragment,
while facet folders use their facet
and technical value. Matching children retain their IDs, load gates, and
compatible cached descendants. Removed nodes and descendants whose expansion
shape changed become stale. Per-record generations prevent requests started
before an ancestor reconciliation from committing obsolete results.

Browser folders use their type, category, and label as semantic identity, keeping
backend node IDs separate from public `NodeId` values. Refreshing a browser folder
first rebuilds its owning object tree to rediscover backend selectors. Cached
browser descendants are invalidated, even if the backend reuses the same numeric
IDs. Deeply nested nodes may become stale and must be reacquired.

`zvfs` models repository hierarchy. Source retrieval, editing, persistence, and
local-file projection belong to higher layers.

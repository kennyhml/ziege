use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zadt::{AdtUri, GlobalWorkbenchType};

/// An opaque node identity scoped to one VFS instance.
///
/// The scope prevents an ID retained by a language-server client from
/// resolving to an unrelated node after reconnecting to another VFS.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct NodeId {
    scope: Uuid,
    index: u64,
}

impl NodeId {
    pub(crate) fn new(scope: Uuid, index: u64) -> Self {
        Self { scope, index }
    }

    pub(crate) fn index_for(self, scope: Uuid) -> Option<u64> {
        (self.scope == scope).then_some(self.index)
    }
}

/// A serializable snapshot of one VFS node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub label: String,
    #[serde(flatten)]
    pub kind: NodeKind,
}

impl Node {
    /// Returns whether this node can have children.
    pub fn is_directory(&self) -> bool {
        match &self.kind {
            NodeKind::Object { object } => object.expandable,
            _ => true,
        }
    }

    /// Returns object metadata when this is a repository-object node.
    pub fn object(&self) -> Option<&ObjectNode> {
        match &self.kind {
            NodeKind::Object { object } => Some(object),
            _ => None,
        }
    }
}

/// The semantic kind and public metadata of a VFS node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NodeKind {
    Root,
    Mount {
        mount: MountKind,
    },
    Package {
        package: String,
        uri: AdtUri,
        object_count: Option<u32>,
    },
    Facet {
        facet: String,
        value: String,
        object_count: u32,
        has_children_of_same_facet: bool,
    },
    Object {
        object: ObjectNode,
    },
    /// A type folder supplied by the backend repository browser, independent of RIS facets.
    ObjectGroup {
        #[serde(rename = "objectType")]
        workbench_type: GlobalWorkbenchType,
        category: String,
    },
}
impl NodeKind {
    pub(crate) fn rank(&self) -> u8 {
        match self {
            NodeKind::Root | NodeKind::Mount { .. } => 0,
            NodeKind::Package { .. } => 1,
            NodeKind::Facet { .. } | NodeKind::ObjectGroup { .. } => 2,
            NodeKind::Object { .. } => 3,
        }
    }
}

/// The behavior represented by a non-package mount node.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MountKind {
    SystemLibrary,
    Selection,
}

/// Serializable metadata for a repository object or source member.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectNode {
    pub name: String,
    /// Known from RIS or an owning parent, absent for unrelated browser references.
    pub package: Option<String>,
    #[serde(rename = "objectType")]
    pub workbench_type: GlobalWorkbenchType,
    pub uri: Option<AdtUri>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
    pub virtual_workbench_uri: Option<String>,
    pub version: Option<String>,
    pub expandable: bool,
    pub description: Option<String>,
}

use std::{
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{path_serializer, project::ProjectNode};

/// Rojo-specific metadata that can be associated with an instance or a snapshot
/// of an instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceMetadata {
    /// Whether instances not present in the source should be ignored when
    /// live-syncing. This is useful when there are instances that Rojo does not
    /// manage.
    pub ignore_unknown_instances: bool,

    /// If a change occurs to this instance, the instigating source is what
    /// should be run through the snapshot functions to regenerate it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instigating_source: Option<InstigatingSource>,

    /// The paths that, when changed, could cause the function that generated
    /// this snapshot to generate a different snapshot. Paths should be included
    /// even if they don't exist, since the presence of a file can change the
    /// outcome of a snapshot function.
    ///
    /// For example, a file named foo.lua might have these relevant paths:
    /// - foo.lua
    /// - foo.meta.json (even if this file doesn't exist!)
    ///
    /// A directory named bar/ might have these:
    /// - bar/
    /// - bar/init.meta.json
    /// - bar/init.lua
    /// - bar/init.server.lua
    /// - bar/init.client.lua
    /// - bar/default.project.json
    ///
    /// This path is used to make sure that file changes update all instances
    /// that may need updates.
    // TODO: Change this to be a SmallVec for performance in common cases?
    #[serde(serialize_with = "path_serializer::serialize_vec_absolute")]
    pub relevant_paths: Vec<PathBuf>,

    /// Contains information about this instance that should persist between
    /// snapshot invocations and is generally inherited.
    ///
    /// If an instance has a piece of context attached to it, then the next time
    /// that instance's instigating source is snapshotted directly, the same
    /// context will be passed into it.
    pub context: InstanceContext,
}

impl InstanceMetadata {
    pub fn new() -> Self {
        Self {
            ignore_unknown_instances: false,
            instigating_source: None,
            relevant_paths: Vec::new(),
            context: InstanceContext::default(),
        }
    }

    pub fn ignore_unknown_instances(self, ignore_unknown_instances: bool) -> Self {
        Self {
            ignore_unknown_instances,
            ..self
        }
    }

    pub fn instigating_source(self, instigating_source: impl Into<InstigatingSource>) -> Self {
        Self {
            instigating_source: Some(instigating_source.into()),
            ..self
        }
    }

    pub fn relevant_paths(self, relevant_paths: Vec<PathBuf>) -> Self {
        Self {
            relevant_paths,
            ..self
        }
    }

    pub fn context(self, context: &InstanceContext) -> Self {
        Self {
            context: context.clone(),
            ..self
        }
    }
}

impl Default for InstanceMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceContext {}

impl Default for InstanceContext {
    fn default() -> Self {
        InstanceContext {}
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum InstigatingSource {
    Path(#[serde(serialize_with = "path_serializer::serialize_absolute")] PathBuf),
    ProjectNode(
        #[serde(serialize_with = "path_serializer::serialize_absolute")] PathBuf,
        String,
        ProjectNode,
    ),
}

impl fmt::Debug for InstigatingSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstigatingSource::Path(path) => write!(formatter, "Path({})", path.display()),
            InstigatingSource::ProjectNode(path, name, node) => write!(
                formatter,
                "ProjectNode({}: {:?}) from path {}",
                name,
                node,
                path.display()
            ),
        }
    }
}

impl From<PathBuf> for InstigatingSource {
    fn from(path: PathBuf) -> Self {
        InstigatingSource::Path(path)
    }
}

impl From<&Path> for InstigatingSource {
    fn from(path: &Path) -> Self {
        InstigatingSource::Path(path.to_path_buf())
    }
}

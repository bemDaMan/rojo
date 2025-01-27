use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use rbx_dom_weak::UnresolvedRbxValue;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

pub static PROJECT_FILENAME: &str = "default.project.json";

/// Error type returned by any function that handles projects.
#[derive(Debug, Snafu)]
pub struct ProjectError(Error);

#[derive(Debug, Snafu)]
enum Error {
    /// A general IO error occurred.
    Io { source: io::Error, path: PathBuf },

    /// An error with JSON parsing occurred.
    Json {
        source: serde_json::Error,
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Project {
    /// The name of the top-level instance described by the project.
    pub name: String,

    /// The tree of instances described by this project. Projects always
    /// describe at least one instance.
    pub tree: ProjectNode,

    /// If specified, sets the default port that `rojo serve` should use when
    /// using this project for live sync.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serve_port: Option<u16>,

    /// If specified, contains the set of place IDs that this project is
    /// compatible with when doing live sync.
    ///
    /// This setting is intended to help prevent syncing a Rojo project into the
    /// wrong Roblox place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serve_place_ids: Option<HashSet<u64>>,

    /// The path to the file that this project came from. Relative paths in the
    /// project should be considered relative to the parent of this field, also
    /// given by `Project::folder_location`.
    #[serde(skip)]
    pub file_location: PathBuf,
}

impl Project {
    /// Tells whether the given path describes a Rojo project.
    pub fn is_project_file(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(".project.json"))
            .unwrap_or(false)
    }

    /// Attempt to locate a project represented by the given path.
    ///
    /// This will find a project if the path refers to a `.project.json` file,
    /// or is a folder that contains a `default.project.json` file.
    fn locate(path: &Path) -> Option<PathBuf> {
        let meta = fs::metadata(path).ok()?;

        if meta.is_file() {
            if Project::is_project_file(path) {
                Some(path.to_path_buf())
            } else {
                None
            }
        } else {
            let child_path = path.join(PROJECT_FILENAME);
            let child_meta = fs::metadata(&child_path).ok()?;

            if child_meta.is_file() {
                Some(child_path)
            } else {
                // This is a folder with the same name as a Rojo default project
                // file.
                //
                // That's pretty weird, but we can roll with it.
                None
            }
        }
    }

    pub fn load_from_slice(
        contents: &[u8],
        project_file_location: &Path,
    ) -> Result<Self, serde_json::Error> {
        let mut project: Self = serde_json::from_slice(&contents)?;
        project.file_location = project_file_location.to_path_buf();
        project.check_compatibility();
        Ok(project)
    }

    pub fn load_fuzzy(fuzzy_project_location: &Path) -> Result<Option<Self>, ProjectError> {
        if let Some(project_path) = Self::locate(fuzzy_project_location) {
            let project = Self::load_exact(&project_path)?;

            Ok(Some(project))
        } else {
            Ok(None)
        }
    }

    fn load_exact(project_file_location: &Path) -> Result<Self, ProjectError> {
        let contents = fs::read_to_string(project_file_location).context(Io {
            path: project_file_location,
        })?;

        let mut project: Project = serde_json::from_str(&contents).context(Json {
            path: project_file_location,
        })?;

        project.file_location = project_file_location.to_path_buf();
        project.check_compatibility();

        Ok(project)
    }

    pub fn save(&self) -> Result<(), ProjectError> {
        unimplemented!()
    }

    /// Checks if there are any compatibility issues with this project file and
    /// warns the user if there are any.
    fn check_compatibility(&self) {
        self.tree.validate_reserved_names();
    }

    pub fn folder_location(&self) -> &Path {
        self.file_location.parent().unwrap()
    }
}

/// Describes an instance and its descendants in a project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProjectNode {
    /// If set, defines the ClassName of the described instance.
    ///
    /// `$className` MUST be set if `$path` is not set.
    ///
    /// `$className` CANNOT be set if `$path` is set and the instance described
    /// by that path has a ClassName other than Folder.
    #[serde(rename = "$className", skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,

    /// Contains all of the children of the described instance.
    #[serde(flatten)]
    pub children: BTreeMap<String, ProjectNode>,

    /// The properties that will be assigned to the resulting instance.
    ///
    // TODO: Is this legal to set if $path is set?
    #[serde(
        rename = "$properties",
        default,
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub properties: HashMap<String, UnresolvedRbxValue>,

    /// Defines the behavior when Rojo encounters unknown instances in Roblox
    /// Studio during live sync. `$ignoreUnknownInstances` should be considered
    /// a large hammer and used with care.
    ///
    /// If set to `true`, those instances will be left alone. This may cause
    /// issues when files that turn into instances are removed while Rojo is not
    /// running.
    ///
    /// If set to `false`, Rojo will destroy any instances it does not
    /// recognize.
    ///
    /// If unset, its default value depends on other settings:
    /// - If `$path` is not set, defaults to `true`
    /// - If `$path` is set, defaults to `false`
    #[serde(
        rename = "$ignoreUnknownInstances",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_unknown_instances: Option<bool>,

    /// Defines that this instance should come from the given file path. This
    /// path can point to any file type supported by Rojo, including Lua files
    /// (`.lua`), Roblox models (`.rbxm`, `.rbxmx`), and localization table
    /// spreadsheets (`.csv`).
    #[serde(
        rename = "$path",
        serialize_with = "crate::path_serializer::serialize_option_absolute",
        skip_serializing_if = "Option::is_none"
    )]
    pub path: Option<PathBuf>,
}

impl ProjectNode {
    fn validate_reserved_names(&self) {
        for (name, child) in &self.children {
            if name.starts_with('$') {
                log::warn!(
                    "Keys starting with '$' are reserved by Rojo to ensure forward compatibility."
                );
                log::warn!(
                    "This project uses the key '{}', which should be renamed.",
                    name
                );
            }

            child.validate_reserved_names();
        }
    }
}

//! The plugin registry and the full rule catalog.

use crate::config::ForeignRuleNames;
use crate::generic::{generic_set, GenericRule, GenericSet, MetaRule};
use mote_core::{LanguagePlugin, ParamValue, Quality, RuleKind, RuleMeta, RuleStatus, Severity};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Where a rule is implemented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleOrigin {
    /// Implemented by a language plugin.
    Plugin,
    /// Engine rule instantiated per language (complexity, TODOs, duplication, …).
    Generic,
    /// Engine meta rule about the analysis itself (`mote/*`).
    Meta,
}

/// One entry of the rule catalog.
#[derive(Clone, Copy, Debug)]
pub struct CatalogEntry {
    pub meta: &'static RuleMeta,
    pub origin: RuleOrigin,
    /// Index of the plugin the rule belongs to (`None` for meta rules).
    pub plugin: Option<usize>,
    /// The generic rule, for [`RuleOrigin::Generic`].
    pub generic: Option<GenericRule>,
}

/// Errors building a registry.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("two plugins use the id `{0}`")]
    DuplicatePlugin(String),
    #[error("file extension `.{ext}` is claimed by both `{first}` and `{second}`")]
    DuplicateExtension {
        ext: String,
        first: String,
        second: String,
    },
    #[error("rule id `{0}` is defined twice")]
    DuplicateRule(String),
    #[error("rule `{rule}` of plugin `{plugin}` must be named `{plugin}/<name>`")]
    BadRuleId { rule: String, plugin: String },
}

/// Holds the language plugins and the rule catalog built from them.
///
/// The catalog contains each plugin's own rules, the engine's generic rules instantiated for
/// every plugin language (unless the plugin implements a rule with the same id itself), and the
/// `mote/*` meta rules.
pub struct Registry {
    plugins: Vec<Box<dyn LanguagePlugin>>,
    by_ext: FxHashMap<String, usize>,
    catalog: Vec<CatalogEntry>,
    index: FxHashMap<&'static str, usize>,
    /// Catalog indices applicable to files of each plugin (own + generic + meta), sorted by id.
    per_plugin: Vec<Vec<usize>>,
    generic_sets: Vec<&'static GenericSet>,
    /// Generic rules each plugin implements itself.
    overridden: Vec<Vec<GenericRule>>,
    /// Other tools' rule names, for configuration errors.
    foreign: Option<Box<dyn ForeignRuleNames>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field(
                "plugins",
                &self.plugins.iter().map(|p| p.id()).collect::<Vec<_>>(),
            )
            .field("rules", &self.catalog.len())
            .finish()
    }
}

impl Registry {
    /// Build a registry from plugins. Fails on duplicate plugin ids, extensions, or rule ids.
    pub fn new(plugins: Vec<Box<dyn LanguagePlugin>>) -> Result<Registry, RegistryError> {
        let mut by_ext: FxHashMap<String, usize> = FxHashMap::default();
        let mut catalog: Vec<CatalogEntry> = Vec::new();
        let mut index: FxHashMap<&'static str, usize> = FxHashMap::default();
        let mut per_plugin = Vec::with_capacity(plugins.len());
        let mut generic_sets = Vec::with_capacity(plugins.len());
        let mut overridden_all = Vec::with_capacity(plugins.len());

        let mut add = |entry: CatalogEntry,
                       catalog: &mut Vec<CatalogEntry>|
         -> Result<usize, RegistryError> {
            if index.contains_key(entry.meta.id) {
                return Err(RegistryError::DuplicateRule(entry.meta.id.to_string()));
            }
            catalog.push(entry);
            index.insert(entry.meta.id, catalog.len() - 1);
            Ok(catalog.len() - 1)
        };

        let mut meta_idx = Vec::new();
        for m in MetaRule::ALL {
            meta_idx.push(add(
                CatalogEntry {
                    meta: m.meta(),
                    origin: RuleOrigin::Meta,
                    plugin: None,
                    generic: None,
                },
                &mut catalog,
            )?);
        }

        let mut seen_ids: Vec<&str> = Vec::new();
        for (pi, plugin) in plugins.iter().enumerate() {
            let pid = plugin.id();
            if seen_ids.contains(&pid) || pid == "mote" {
                return Err(RegistryError::DuplicatePlugin(pid.to_string()));
            }
            seen_ids.push(pid);
            for ext in plugin.extensions() {
                let ext = ext.trim_start_matches('.').to_ascii_lowercase();
                if let Some(&other) = by_ext.get(&ext) {
                    return Err(RegistryError::DuplicateExtension {
                        ext,
                        first: plugins[other].id().to_string(),
                        second: pid.to_string(),
                    });
                }
                by_ext.insert(ext, pi);
            }
            let mut mine = meta_idx.clone();
            let prefix = format!("{pid}/");
            let own = plugin.rules();
            for meta in &own {
                if !meta.id.starts_with(&prefix) || meta.id.len() == prefix.len() {
                    return Err(RegistryError::BadRuleId {
                        rule: meta.id.to_string(),
                        plugin: pid.to_string(),
                    });
                }
                mine.push(add(
                    CatalogEntry {
                        meta,
                        origin: RuleOrigin::Plugin,
                        plugin: Some(pi),
                        generic: None,
                    },
                    &mut catalog,
                )?);
            }
            let set = generic_set(pid);
            let mut overridden = Vec::new();
            for &(rule, meta) in &set.rules {
                if own.iter().any(|m| m.id == meta.id) {
                    overridden.push(rule);
                    continue;
                }
                mine.push(add(
                    CatalogEntry {
                        meta,
                        origin: RuleOrigin::Generic,
                        plugin: Some(pi),
                        generic: Some(rule),
                    },
                    &mut catalog,
                )?);
            }
            per_plugin.push(mine);
            generic_sets.push(set);
            overridden_all.push(overridden);
        }

        // Sort per-plugin lists by id for deterministic iteration.
        for list in &mut per_plugin {
            list.sort_by_key(|&i| catalog[i].meta.id);
        }
        Ok(Registry {
            plugins,
            by_ext,
            catalog,
            index,
            per_plugin,
            generic_sets,
            overridden: overridden_all,
            foreign: None,
        })
    }

    /// Explain other tools' rule names (ESLint, PMD) written in a configuration's rule maps
    /// with `names` (see [`ForeignRuleNames`]).
    pub fn with_foreign_rule_names(mut self, names: Box<dyn ForeignRuleNames>) -> Registry {
        self.foreign = Some(names);
        self
    }

    /// The other tools' rule names registered with
    /// [`with_foreign_rule_names`](Self::with_foreign_rule_names).
    pub fn foreign_rule_names(&self) -> Option<&dyn ForeignRuleNames> {
        self.foreign.as_deref()
    }

    /// The registered plugins, in registration order.
    pub fn plugins(&self) -> &[Box<dyn LanguagePlugin>] {
        &self.plugins
    }

    /// Plugin `index`.
    pub fn plugin(&self, index: usize) -> &dyn LanguagePlugin {
        self.plugins[index].as_ref()
    }

    /// Index of the plugin with this id.
    pub fn plugin_index(&self, id: &str) -> Option<usize> {
        self.plugins.iter().position(|p| p.id() == id)
    }

    /// The plugin handling `path` (by extension, case-insensitive), if any.
    pub fn plugin_for_path(&self, path: &str) -> Option<usize> {
        let name = path.rsplit('/').next().unwrap_or(path);
        let (_, ext) = name.rsplit_once('.')?;
        if ext.is_empty() {
            return None;
        }
        if ext.bytes().all(|b| !b.is_ascii_uppercase()) {
            self.by_ext.get(ext).copied()
        } else {
            self.by_ext.get(&ext.to_ascii_lowercase()).copied()
        }
    }

    /// The full catalog in registration order (meta rules first, then per plugin).
    pub fn catalog(&self) -> &[CatalogEntry] {
        &self.catalog
    }

    /// Look up a rule by id.
    pub fn rule(&self, id: &str) -> Option<&CatalogEntry> {
        self.index.get(id).map(|&i| &self.catalog[i])
    }

    /// Catalog entries applicable to files of plugin `index`, sorted by id.
    pub fn rules_for_plugin(&self, index: usize) -> impl Iterator<Item = &CatalogEntry> {
        self.per_plugin[index].iter().map(|&i| &self.catalog[i])
    }

    /// Generic rule ids for plugin `index`.
    pub fn generic_set(&self, index: usize) -> &'static GenericSet {
        self.generic_sets[index]
    }

    /// Generic rules plugin `index` implements itself.
    pub fn overridden_generics(&self, index: usize) -> &[GenericRule] {
        &self.overridden[index]
    }

    /// All rule ids, sorted.
    pub fn rule_ids(&self) -> Vec<&'static str> {
        let mut ids: Vec<&'static str> = self.catalog.iter().map(|e| e.meta.id).collect();
        ids.sort_unstable();
        ids
    }

    /// Serializable description of a rule.
    pub fn rule_info(&self, id: &str) -> Option<RuleInfo> {
        self.rule(id).map(RuleInfo::from_entry)
    }
}

/// Serializable rule metadata (mirror of [`RuleMeta`] for reports, SARIF and JSON output).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct RuleInfo {
    pub id: String,
    pub name: String,
    pub language: String,
    pub kind: RuleKind,
    pub quality: Quality,
    /// Default severity (config may override it per file).
    pub severity: Severity,
    pub status: RuleStatus,
    pub default_enabled: bool,
    pub tags: Vec<String>,
    pub params: Vec<ParamInfo>,
    /// Markdown description.
    pub description: String,
    pub remediation_minutes: u32,
    pub origin: RuleOrigin,
    /// The rule offers fixes (`mote fix`) for its findings. An addition: false when read from
    /// results written before it existed.
    #[serde(default)]
    pub fixable: bool,
}

/// A rule parameter with its default.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct ParamInfo {
    pub name: String,
    pub default: ParamValue,
    pub description: String,
}

impl RuleInfo {
    /// Build from a catalog entry.
    pub fn from_entry(e: &CatalogEntry) -> RuleInfo {
        let m = e.meta;
        RuleInfo {
            id: m.id.to_string(),
            name: m.name.to_string(),
            language: m.language.to_string(),
            kind: m.kind,
            quality: m.quality,
            severity: m.severity,
            status: m.status,
            default_enabled: m.default_enabled,
            tags: m.tags.iter().map(|t| t.to_string()).collect(),
            params: m
                .params
                .iter()
                .map(|p| ParamInfo {
                    name: p.name.to_string(),
                    default: p.default.to_value(),
                    description: p.description.to_string(),
                })
                .collect(),
            description: m.description.to_string(),
            remediation_minutes: m.remediation_minutes,
            origin: e.origin,
            fixable: m.fixable,
        }
    }
}

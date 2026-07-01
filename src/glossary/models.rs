use serde::{Deserialize, Serialize};

/// A field declared on a domain entity. `type_name` is one of `int`, `bool`,
/// `string`, `enum`, or `""` (unspecified). When `enum`, `enum_values` lists the
/// permitted members.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub type_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
    /// A determinant / lifecycle field: it holds one of a bounded range of
    /// states and selects which guarded transition applies. Declared in the
    /// `states:` section of a glossary block.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub state: bool,
}

impl FieldDef {
    /// A plain (non-state) field with an optional type.
    pub fn new(name: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            enum_values: Vec::new(),
            state: false,
        }
    }
}

/// A domain entity: its canonical name, the instance-prefix aliases used in
/// contract field paths (e.g. `loan` for `Mortgage`), a prose description, and
/// its declared fields and operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aka: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub describes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<String>,
}

impl EntityDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            aka: Vec::new(),
            describes: None,
            fields: Vec::new(),
            operations: Vec::new(),
        }
    }

    /// Look up a field by name.
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Whether `prefix` is one of this entity's instance aliases.
    pub fn has_alias(&self, prefix: &str) -> bool {
        self.aka.iter().any(|a| a == prefix)
    }

    /// The determinant / lifecycle fields (declared under `states:`).
    pub fn state_fields(&self) -> impl Iterator<Item = &FieldDef> {
        self.fields.iter().filter(|f| f.state)
    }

    /// Whether any state field is declared for this entity.
    pub fn has_state(&self) -> bool {
        self.fields.iter().any(|f| f.state)
    }
}

/// The domain glossary: the set of entities the rule vocabulary is drawn from.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Glossary {
    pub entities: Vec<EntityDef>,
}

impl Glossary {
    pub fn new(entities: Vec<EntityDef>) -> Self {
        Self { entities }
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Entity by canonical name (exact).
    pub fn entity(&self, name: &str) -> Option<&EntityDef> {
        self.entities.iter().find(|e| e.name == name)
    }

    /// Entity that declares `prefix` as one of its instance aliases.
    pub fn entity_by_alias(&self, prefix: &str) -> Option<&EntityDef> {
        self.entities.iter().find(|e| e.has_alias(prefix))
    }
}

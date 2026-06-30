fn normalize_slug(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub default_branch: String,
    pub contract_store_type: String,
}

impl Project {
    pub fn new(name: impl Into<String>, slug: impl Into<String>) -> Self {
        let slug = normalize_slug(&slug.into());
        Self {
            name: name.into(),
            slug,
            description: String::new(),
            default_branch: "main".to_owned(),
            contract_store_type: "local".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractDocument {
    pub project_slug: String,
    pub path: String,
    pub title: String,
    pub markdown_content: String,
    pub version: u32,
}

impl ContractDocument {
    pub fn new(
        project_slug: impl Into<String>,
        path: impl Into<String>,
        title: impl Into<String>,
        markdown_content: impl Into<String>,
    ) -> Self {
        Self {
            project_slug: normalize_slug(&project_slug.into()),
            path: path.into(),
            title: title.into(),
            markdown_content: markdown_content.into(),
            version: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_slug_normalized_to_lowercase_hyphen() {
        let p = Project::new("My Project", "My Project");
        assert_eq!(p.slug, "my-project");
    }

    #[test]
    fn project_default_branch_is_main() {
        let p = Project::new("X", "x");
        assert_eq!(p.default_branch, "main");
    }

    #[test]
    fn contract_document_slug_normalized() {
        let d = ContractDocument::new("My Project", "orders.md", "Orders", "# Orders");
        assert_eq!(d.project_slug, "my-project");
    }

    #[test]
    fn contract_document_version_defaults_to_1() {
        let d = ContractDocument::new("proj", "f.md", "T", "content");
        assert_eq!(d.version, 1);
    }
}

use std::collections::HashMap;
#[derive(Clone, Debug, Default)]
pub struct TopicAcl {
    rules: HashMap<String, Vec<String>>,
}
impl TopicAcl {
    pub fn from_env() -> Result<Self, String> {
        let raw = std::env::var("RELAY_ACL")
            .unwrap_or_else(|_| "clap-provider-1=clap.embedding.request".to_string());
        Self::parse(&raw)
    }
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut rules = HashMap::new();
        for entry in raw
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let (node, topics) = entry
                .split_once('=')
                .ok_or_else(|| format!("invalid ACL entry `{entry}`; expected node=topic|topic"))?;
            let node = node.trim();
            if node.is_empty() {
                return Err("ACL node ID must not be empty".into());
            }
            let parsed: Vec<String> = topics
                .split('|')
                .map(str::trim)
                .filter(|topic| !topic.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            if parsed.is_empty() {
                return Err(format!("ACL entry for `{node}` has no topics"));
            }
            rules.insert(node.to_owned(), parsed);
        }
        if rules.is_empty() {
            return Err("ACL is empty; refusing open access".into());
        }
        Ok(Self { rules })
    }
    pub fn allows(&self, node_id: &str, topic: &str) -> bool {
        self.rules
            .get(node_id)
            .map(|topics| topics.iter().any(|allowed| topic_matches(allowed, topic)))
            .unwrap_or(false)
    }
}
fn topic_matches(pattern: &str, topic: &str) -> bool {
    if pattern == topic || pattern == "*" {
        return true;
    }
    pattern.ends_with(".*") && topic.starts_with(&pattern[..pattern.len() - 1])
}
#[cfg(test)]
mod tests {
    use super::TopicAcl;
    #[test]
    fn exact_and_namespace_rules_are_enforced() {
        let acl = TopicAcl::parse("node-a=clap.embedding.request|metrics.*").unwrap();
        assert!(acl.allows("node-a", "clap.embedding.request"));
        assert!(acl.allows("node-a", "metrics.health"));
        assert!(!acl.allows("node-a", "ledger.settle"));
        assert!(!acl.allows("node-b", "metrics.health"));
    }
    #[test]
    fn malformed_or_empty_policy_fails_closed() {
        assert!(TopicAcl::parse("").is_err());
        assert!(TopicAcl::parse("node-a").is_err());
    }
}

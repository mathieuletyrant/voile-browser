use crate::model::LiveItem;
use std::collections::BTreeMap;

/// Fusionne un résultat frais avec ce que l'utilisateur a déjà vu :
/// un élément est « non lu » s'il est nouveau ou a bougé depuis son ouverture.
/// L'ordre de la source est conservé (GitHub trie par mise à jour).
pub fn merge_live_items(fresh: Vec<LiveItem>, seen: &BTreeMap<String, String>) -> Vec<LiveItem> {
    fresh
        .into_iter()
        .map(|mut item| {
            item.unread = seen.get(&item.key) != Some(&item.updated_at);
            item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, updated: &str) -> LiveItem {
        LiveItem {
            key: key.into(),
            title: key.into(),
            url: format!("https://github.com/{key}"),
            subtitle: String::new(),
            updated_at: updated.into(),
            badges: vec![],
            avatar: None,
            unread: false,
        }
    }

    #[test]
    fn new_and_updated_items_are_unread() {
        let seen = BTreeMap::from([
            ("a/b#1".to_string(), "2026-11-01T10:00:00Z".to_string()),
            ("a/b#2".to_string(), "2026-11-01T10:00:00Z".to_string()),
        ]);
        let merged = merge_live_items(
            vec![
                item("a/b#1", "2026-11-01T10:00:00Z"),
                item("a/b#2", "2026-11-02T09:00:00Z"),
                item("a/b#3", "2026-11-02T09:00:00Z"),
            ],
            &seen,
        );
        let unread: Vec<_> = merged.iter().map(|i| i.unread).collect();
        assert_eq!(unread, [false, true, true]);
    }
}

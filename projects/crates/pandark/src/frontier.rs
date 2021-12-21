use std::collections::HashSet;

use pandark_types::{FrontierItem, LinkCandidate, LinkRelation};

/// Bounded frontier queue with URL deduplication.
#[derive(Debug, Clone, Default)]
pub struct FrontierQueue {
    seen: HashSet<String>,
    pending: Vec<FrontierItem>,
}

impl FrontierQueue {
    /// Empty frontier queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the canonical request URL is already known.
    pub fn contains(&self, request_url: &str) -> bool {
        self.seen.contains(request_url)
    }

    /// Enqueue an item when not yet seen.
    pub fn push(&mut self, item: FrontierItem) -> bool {
        let key = item.request_url.to_string();
        if !self.seen.insert(key) {
            return false;
        }
        self.pending.push(item);
        true
    }

    /// Pop the next frontier item by priority then discovery time.
    pub fn pop(&mut self) -> Option<FrontierItem> {
        if self.pending.is_empty() {
            return None;
        }
        let index = self
            .pending
            .iter()
            .enumerate()
            .min_by_key(|(_, item)| (item.priority, item.discovered_at_epoch))
            .map(|(index, _)| index)
            .expect("non-empty pending");
        Some(self.pending.remove(index))
    }

    /// Number of pending items.
    pub fn len(&self) -> usize {
        self.pending.len()
    }
}

/// Convert navigation link candidates into frontier items.
pub fn frontier_from_links(
    queue: &mut FrontierQueue,
    source: &FrontierItem,
    links: &[LinkCandidate],
    max_depth: u32,
) -> Vec<FrontierItem> {
    let mut admitted = Vec::new();
    for link in links {
        if link.relation != LinkRelation::Navigation {
            continue;
        }
        let Ok(url) = pandark_types::canonicalize_request_url(&link.target) else {
            continue;
        };
        if source.depth + 1 > max_depth {
            continue;
        }
        let item = FrontierItem {
            request_url: url.clone(),
            discovery: pandark_types::DiscoverySource::NavigationLink {
                from: source.request_url.clone(),
            },
            depth: source.depth + 1,
            site_key: pandark_types::site_key(&url),
            priority: source.priority + 1,
            attempts: 0,
            discovered_at_epoch: pandark_types::now_epoch(),
        };
        if queue.push(item.clone()) {
            admitted.push(item);
        }
    }
    admitted
}

#[cfg(test)]
mod tests {
    use pandark_types::{CrawlRequest, LinkCandidate, LinkRelation};

    use super::{FrontierQueue, frontier_from_links};
    use crate::admission::seed_frontier;

    #[test]
    fn deduplicates_frontier_items() {
        let mut queue = FrontierQueue::new();
        let request = CrawlRequest::from_seed("https://example.com/a").expect("seed");
        let items = seed_frontier(&request).expect("items");
        assert!(queue.push(items[0].clone()));
        assert!(!queue.push(items[0].clone()));
    }

    #[test]
    fn expands_navigation_links() {
        let mut queue = FrontierQueue::new();
        let request = CrawlRequest::from_seed("https://example.com/").expect("seed");
        let mut items = seed_frontier(&request).expect("items");
        let source = items.remove(0);
        queue.push(source.clone());
        let links = vec![LinkCandidate {
            target: "https://example.com/next".into(),
            relation: LinkRelation::Navigation,
            source_hint: None,
        }];
        let admitted = frontier_from_links(&mut queue, &source, &links, 2);
        assert_eq!(admitted.len(), 1);
        assert_eq!(queue.len(), 2);
    }
}

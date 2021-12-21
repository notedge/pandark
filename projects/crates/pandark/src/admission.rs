use pandark_types::{
    AdmissionDecision, CrawlRequest, CrawlSeed, FrontierItem, Result,
    canonicalize_request_url, site_key,
};

/// Evaluate whether a frontier item may proceed to fetch.
pub fn admit_frontier_item(item: &FrontierItem, request: &CrawlRequest) -> AdmissionDecision {
    if item.request_url.scheme() != "http"
        && item.request_url.scheme() != "https"
        && item.request_url.scheme() != "file"
    {
        return AdmissionDecision::Rejected {
            reason: "unsupported-scheme".into(),
        };
    }

    if item.depth > request.budget.max_depth {
        return AdmissionDecision::Rejected {
            reason: "depth-exceeded".into(),
        };
    }

    if item.attempts > 0 && item.attempts > 3 {
        return AdmissionDecision::Rejected {
            reason: "retry-exhausted".into(),
        };
    }

    AdmissionDecision::Admitted
}

/// Build seed frontier items from a crawl request.
pub fn seed_frontier(request: &CrawlRequest) -> Result<Vec<FrontierItem>> {
    let mut items = Vec::with_capacity(request.seeds.len());
    for seed in &request.seeds {
        items.push(seed_to_frontier(seed)?);
    }
    Ok(items)
}

fn seed_to_frontier(seed: &CrawlSeed) -> Result<FrontierItem> {
    let request_url = canonicalize_request_url(seed.url.as_str())?;
    Ok(FrontierItem::seed(
        request_url.clone(),
        site_key(&request_url),
    ))
}

#[cfg(test)]
mod tests {
    use pandark_types::{AdmissionDecision, CrawlRequest};

    use super::{admit_frontier_item, seed_frontier};

    #[test]
    fn rejects_items_beyond_depth() {
        let request = CrawlRequest::from_seed("https://example.com").expect("seed");
        let mut items = seed_frontier(&request).expect("frontier");
        items[0].depth = request.budget.max_depth + 1;
        assert!(matches!(
            admit_frontier_item(&items[0], &request),
            AdmissionDecision::Rejected { .. }
        ));
    }
}

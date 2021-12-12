use pandark_types::{CrawlError, ExtractStatus, PageTransaction, Result};

/// Validate and commit a page transaction.
pub fn commit_page(transaction: PageTransaction) -> Result<String> {
    match transaction.extract.status {
        ExtractStatus::Failed | ExtractStatus::Unsupported => {
            return Err(CrawlError::InvalidInput(format!(
                "page `{}` cannot be committed with status `{}`",
                transaction.page_identity,
                status_label(transaction.extract.status)
            )));
        }
        ExtractStatus::Complete | ExtractStatus::Partial => {}
    }

    let document = transaction
        .extract
        .document
        .as_ref()
        .ok_or_else(|| CrawlError::InvalidInput("missing document graph".into()))?;
    let validation = document.validate();
    if !validation.is_valid() {
        return Err(CrawlError::InvalidInput(format!(
            "document validation failed for `{}`",
            transaction.page_identity
        )));
    }

    Ok(transaction.page_identity)
}

fn status_label(status: ExtractStatus) -> &'static str {
    match status {
        ExtractStatus::Complete => "complete",
        ExtractStatus::Partial => "partial",
        ExtractStatus::Unsupported => "unsupported",
        ExtractStatus::Failed => "failed",
    }
}

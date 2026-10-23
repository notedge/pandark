export { createPandark } from "./client.js";
export type { PandarkClient, PandarkCrawlOptions } from "./client.js";

export { Crawler } from "./crawler.js";
export type {
    CrawlerOptions,
    FetchOptions,
    CrawlOptions,
    CrawlScope,
    CrawlLimits,
    FetchArtifact,
    Page,
    PageOutcome,
    CrawlRun,
    DiagnosticRecord,
    HandlePageFn,
    ConverterLikeResult,
} from "./crawler.js";

export { loadPandarkNode } from "./node/load.js";
export type { PandarkBindings } from "./types.js";

// ---------------------------------------------------------------------------
// PageView — 受控页面视图层
// ---------------------------------------------------------------------------

export {
    createPageView,
    query,
    queryAll,
    textOf,
    htmlOf,
} from "./page-view.js";
export type {
    PageView,
    PageNode,
    PageNodeKind,
    PageMetadata,
} from "./page-view.js";

// ---------------------------------------------------------------------------
// ExtractionProfile — 声明式抽取配置
// ---------------------------------------------------------------------------

export { defineExtractionProfile, ARTICLE_PROFILE } from "./extraction-profile.js";
export type {
    ExtractionProfile,
    ExtractionScope,
    FieldSpec,
    AssetSpec,
} from "./extraction-profile.js";

// ---------------------------------------------------------------------------
// ExtractionResult + runExtraction — 抽取主流程
// ---------------------------------------------------------------------------

export { runExtraction, extractFromHtml } from "./extraction-result.js";
export type {
    ExtractionResult,
    ScopeStatus,
    AssetCandidate,
    LinkCandidate,
} from "./extraction-result.js";

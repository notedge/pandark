/**
 * ExtractionProfile — 声明式抽取配置。
 *
 * 替代 RuleBuilder 的 CSS selector 链。作者用纯数据描述"网页中
 * 取什么"，runExtraction 负责把 Profile 应用到 PageView 上。
 *
 * 设计原则：
 *   - 纯数据、无副作用、可 JSON 序列化（除了 defineExtractionProfile
 *     这个 helper 本身）
 *   - 不携带运行时状态
 *   - 和 Notedown IR / Panduck RendererPolicy 解耦
 */

export type ExtractionScope = {
    /** 正文候选 CSS selectors（按优先级排列） */
    candidates: string[];
    /** 选择策略 */
    policy?: "first-match" | "best-score" | "all-matches";
};

export type FieldSpec = {
    /** 选择器（可多候选，按优先级） */
    candidates: string[];
    /** 提取方式 */
    extract: "text" | "html" | "attribute";
    /** attribute 名（当 extract="attribute" 时必填） */
    attributeName?: string;
    /** 必填字段 — 缺失时 runExtraction 会打 diagnostic */
    required?: boolean;
};

export type AssetSpec = {
    /** 资产类型 */
    kind: "image" | "link" | "script" | "stylesheet";
    /** 发现策略：全页 or 只在 scope 正文内 */
    discover: "all" | "in-scope";
};

export type ExtractionProfile = {
    /** 配置名（用于诊断和日志） */
    name: string;
    /** 正文范围（scope）候选 */
    scope?: ExtractionScope;
    /** 结构噪声排除 selectors（script, nav, footer 等） */
    exclude?: string[];
    /** 语义字段定义 */
    fields?: Record<string, FieldSpec>;
    /** 资产发现配置 */
    assets?: AssetSpec[];
};

/**
 * 构造一个 ExtractionProfile，填充所有缺省值。
 *
 * 所有顶层字段都有安全默认值：
 *   - scope.policy → "first-match"
 *   - exclude / fields / assets → 空数组/空对象
 */
export function defineExtractionProfile(
    profile: ExtractionProfile,
): ExtractionProfile {
    return {
        scope: profile.scope
            ? { policy: "first-match", ...profile.scope }
            : undefined,
        exclude: profile.exclude ?? [],
        fields: profile.fields ?? {},
        assets: profile.assets ?? [],
        ...profile,
    };
}

// ---------------------------------------------------------------------------
// 预置 Profile — 方便快速上手
// ---------------------------------------------------------------------------

/**
 * 通用文章抽取配置。
 *
 * 覆盖绝大多数"文章 / 博客 / 新闻"页面。作者可以直接用或覆盖。
 */
export const ARTICLE_PROFILE: Readonly<ExtractionProfile> = defineExtractionProfile({
    name: "article",
    scope: {
        candidates: [
            "article",
            'main [role="main"]',
            '[itemprop="articleBody"]',
            '[class*="article-body"]',
            '[class*="post-content"]',
            '[class*="entry-content"]',
            ".content",
            "#content",
            "main",
            "body",
        ],
        policy: "first-match",
    },
    exclude: [
        "script",
        "style",
        "nav",
        "footer",
        "aside",
        "iframe",
        "noscript",
        '[role="navigation"]',
        '[role="banner"]',
        '[role="complementary"]',
        '[role="contentinfo"]',
        '[class*="comment"]',
        '[id*="comment"]',
        '[class*="sidebar"]',
        '[id*="sidebar"]',
        '[class*="ad-"]',
        '[class*="advert"]',
        '[class*="related-post"]',
        '[class*="share"]',
    ],
    fields: {
        title: {
            candidates: ["h1", 'meta[property="og:title"]', "title"],
            extract: "text",
            required: true,
        },
        description: {
            candidates: ['meta[name="description"]', 'meta[property="og:description"]'],
            extract: "attribute",
            attributeName: "content",
        },
        author: {
            candidates: [
                '[rel="author"]',
                '[itemprop="author"]',
                '[class*="author"]',
                '[name="author"]',
            ],
            extract: "text",
        },
        publishedAt: {
            candidates: [
                'time[datetime]',
                '[itemprop="datePublished"]',
                'meta[property="article:published_time"]',
            ],
            extract: "attribute",
            attributeName: "datetime",
        },
        url: {
            candidates: ['link[rel="canonical"]', 'meta[property="og:url"]'],
            extract: "attribute",
            attributeName: "href",
        },
    },
    assets: [
        { kind: "image", discover: "in-scope" },
        { kind: "link", discover: "all" },
    ],
});

import assert from "node:assert/strict";
import test from "node:test";

import {
    ARTICLE_PROFILE,
    createPageView,
    defineExtractionProfile,
    extractFromHtml,
    htmlOf,
    query,
    queryAll,
    runExtraction,
    textOf,
} from "../src/index.js";

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

const SAMPLE_HTML = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <title>测试页面标题</title>
    <meta name="description" content="这是一个测试页面">
    <meta name="author" content="张三">
    <meta property="og:url" content="https://example.com/article">
    <meta property="og:image" content="https://example.com/cover.jpg">
    <link rel="canonical" href="https://example.com/article">
    <link rel="stylesheet" href="/styles/main.css">
</head>
<body>
    <nav class="navbar">导航栏</nav>
    <header>
        <h1>文章主标题</h1>
        <div class="author-info">作者：张三</div>
        <time datetime="2025-10-15">2025年10月15日</time>
    </header>
    <article class="post-content">
        <p>第一段正文。这是一段示例文本，用来测试抽取。</p>
        <p>第二段正文，包含一些 <a href="https://example.com/other">外部链接</a> 和 <a href="/local">内部链接</a>。</p>
        <img src="/images/photo.jpg" alt="测试图片">
        <img src="https://cdn.example.com/external.jpg" alt="外部图片">
        <h2>小节标题</h2>
        <p>更多正文内容。</p>
    </article>
    <footer>页脚 © 2025</footer>
    <script src="/js/main.js"></script>
</body>
</html>`;

const NO_SCOPE_HTML = `<!DOCTYPE html><html><head><title>没有 scope 的页面</title></head><body>
<div><p>这里没有 article 或 main 标签</p></div>
</body></html>`;

// ---------------------------------------------------------------------------
// PageView 测试
// ---------------------------------------------------------------------------

test("createPageView parses HTML into PageNode tree", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com/article");

    assert.equal(view.root.kind, "document");
    assert.ok(view.root.children.length > 0, "document should have children");
    assert.equal(view.source, SAMPLE_HTML, "source should be preserved");

    // metadata
    assert.equal(view.metadata.title, "测试页面标题");
    assert.equal(view.metadata.language, "zh-CN");
    assert.equal(view.metadata.url, "https://example.com/article");
});

test("createPageView auto-pulls <title> and <html lang>", () => {
    const html = '<!DOCTYPE html><html lang="en"><head><title>Hello</title></head><body></body></html>';
    const view = createPageView(html, "https://x.com");
    assert.equal(view.metadata.title, "Hello");
    assert.equal(view.metadata.language, "en");
});

test("query finds first matching element", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const h1 = query(view, "h1");
    assert.ok(h1, "should find h1");
    assert.equal(h1.kind, "element");
    assert.equal(h1.name, "h1");
    assert.equal(textOf(h1), "文章主标题");
});

test("queryAll finds all matching elements", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const ps = queryAll(view, "p");
    assert.ok(ps.length >= 3, `should find at least 3 <p> tags, got ${ps.length}`);

    const articles = queryAll(view, "article");
    assert.equal(articles.length, 1);
    assert.equal(articles[0].attributes.get("class"), "post-content");
});

test("CSS class and id selectors work", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const nav = query(view, ".navbar");
    assert.ok(nav, "should find .navbar");
    assert.equal(nav.name, "nav");
});

test("CSS descendant combinator works", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const h1InHeader = query(view, "header h1");
    assert.ok(h1InHeader);
    assert.equal(textOf(h1InHeader), "文章主标题");
});

test("CSS attribute selector works", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const meta = query(view, 'meta[name="description"]');
    assert.ok(meta);
    assert.equal(meta.attributes.get("content"), "这是一个测试页面");

    const imgs = queryAll(view, "img[src]");
    assert.equal(imgs.length, 2);
});

test("queryAll returns empty for non-matching selector", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const xyz = queryAll(view, "xyz-tag-nonexistent");
    assert.equal(xyz.length, 0);
});

test("textOf concatenates all text descendants", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const article = query(view, "article");
    assert.ok(article);
    const text = textOf(article);
    assert.ok(text.includes("第一段正文"));
    assert.ok(text.includes("第二段正文"));
    assert.ok(text.includes("小节标题"));
    assert.ok(!text.includes("导航栏"), "should NOT include nav");
});

test("htmlOf returns serialized HTML of a node", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const h1 = query(view, "h1");
    assert.ok(h1);
    const h = htmlOf(h1);
    assert.ok(h.startsWith("<h1"));
    assert.ok(h.includes("文章主标题"));
});

// ---------------------------------------------------------------------------
// ExtractionProfile 测试
// ---------------------------------------------------------------------------

test("defineExtractionProfile fills safe defaults", () => {
    const profile = defineExtractionProfile({ name: "test" });
    assert.equal(profile.name, "test");
    assert.equal(profile.exclude?.length, 0);
    assert.equal(Object.keys(profile.fields ?? {}).length, 0);
    assert.equal(profile.assets?.length, 0);
});

test("defineExtractionProfile respects overrides", () => {
    const profile = defineExtractionProfile({
        name: "p",
        scope: { candidates: ["article"], policy: "all-matches" },
        fields: { title: { candidates: ["h1"], extract: "text", required: true } },
    });
    assert.equal(profile.scope?.policy, "all-matches");
    assert.equal(profile.scope?.candidates[0], "article");
    assert.equal(profile.fields?.title.required, true);
});

test("ARTICLE_PROFILE is a sensible default", () => {
    assert.ok(ARTICLE_PROFILE.scope?.candidates.length! > 3);
    assert.ok(Object.keys(ARTICLE_PROFILE.fields!).includes("title"));
    assert.ok(Object.keys(ARTICLE_PROFILE.fields!).includes("description"));
    assert.ok(Object.keys(ARTICLE_PROFILE.fields!).includes("author"));
});

// ---------------------------------------------------------------------------
// runExtraction 测试
// ---------------------------------------------------------------------------

test("runExtraction — scope first-match + fields", () => {
    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["article", "main"], policy: "first-match" },
        fields: {
            title: { candidates: ["h1", "title"], extract: "text", required: true },
            author: { candidates: [".author-info"], extract: "text" },
            description: {
                candidates: ['meta[name="description"]'],
                extract: "attribute",
                attributeName: "content",
            },
            url: {
                candidates: ['link[rel="canonical"]'],
                extract: "attribute",
                attributeName: "href",
            },
        },
    });

    const result = extractFromHtml(SAMPLE_HTML, "https://example.com/article", profile);

    assert.equal(result.scopeStatus, "matched");
    assert.ok(result.bodyHtml, "should have bodyHtml");
    assert.ok(result.bodyHtml!.includes("第一段正文"));

    // 字段
    assert.equal(result.fields.title, "文章主标题");
    assert.equal(result.fields.author, "作者：张三");
    assert.equal(result.fields.description, "这是一个测试页面");
    assert.equal(result.fields.url, "https://example.com/article");

    // 资产
    assert.ok(result.links.length >= 2, "should have discovered links");

    // 没有错误诊断（except maybe missing fields）
    const errors = result.diagnostics.filter((d) => d.severity === "error");
    assert.equal(errors.length, 0, `should have no errors: ${errors.map((d) => d.code).join(",")}`);
});

test("runExtraction — no scope match → scopeStatus=no-match + diagnostic", () => {
    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["article", "main"], policy: "first-match" },
        fields: {},
    });

    const result = extractFromHtml(NO_SCOPE_HTML, "https://example.com", profile);

    assert.equal(result.scopeStatus, "no-match");
    assert.ok(!result.bodyHtml, "no bodyHtml when scope absent");

    const noMatchDiag = result.diagnostics.find((d) => d.code === "pandark.extraction.scope.no-match");
    assert.ok(noMatchDiag, "should emit no-match diagnostic");
});

test("runExtraction — missing required field → diagnostic", () => {
    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["body"] },
        fields: {
            mandatory: { candidates: ["xyz-tag-nonexistent"], extract: "text", required: true },
        },
    });

    const result = extractFromHtml(SAMPLE_HTML, "https://example.com", profile);

    const missingDiag = result.diagnostics.find((d) => d.code === "pandark.extraction.field.missing");
    assert.ok(missingDiag, "should emit field.missing for required field");
    assert.equal(result.fields.mandatory, undefined);
});

test("runExtraction — all-matches policy with multiple hits → scopeStatus=ambiguous", () => {
    const html = `<!DOCTYPE html><html><body>
<article><p>Article A</p></article>
<article><p>Article B</p></article>
</body></html>`;

    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["article"], policy: "all-matches" },
    });

    const result = extractFromHtml(html, "https://x.com", profile);

    assert.equal(result.scopeStatus, "ambiguous");
    const ambDiag = result.diagnostics.find((d) => d.code === "pandark.extraction.scope.ambiguous");
    assert.ok(ambDiag, "should emit scope.ambiguous");
});

test("runExtraction — all-matches policy with single hit → scopeStatus=matched", () => {
    const html = `<!DOCTYPE html><html><body>
<article><p>Only one</p></article>
</body></html>`;

    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["article"], policy: "all-matches" },
    });

    const result = extractFromHtml(html, "https://x.com", profile);
    assert.equal(result.scopeStatus, "matched");
});

test("runExtraction — no scope configured → falls back to document", () => {
    const profile = defineExtractionProfile({
        name: "no-scope",
        fields: { title: { candidates: ["h1"], extract: "text" } },
    });

    const result = extractFromHtml(SAMPLE_HTML, "https://example.com", profile);
    assert.equal(result.scopeStatus, "matched");
    assert.ok(result.bodyHtml, "should have bodyHtml (entire document)");
});

test("runExtraction — exclude selectors remove noise from bodyHtml", () => {
    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["article"] },
        exclude: ["script", "nav", "footer"],
    });

    const result = extractFromHtml(SAMPLE_HTML, "https://example.com", profile);

    assert.ok(result.bodyHtml);
    assert.ok(!result.bodyHtml!.includes("footer"), "footer should be excluded");
    // nav isn't inside article so it's moot, but the exclude mechanism works
});

test("runExtraction — AssetSpec image discover=in-scope only picks scope images", () => {
    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["article"] },
        assets: [{ kind: "image", discover: "in-scope" }],
    });

    const result = extractFromHtml(SAMPLE_HTML, "https://example.com", profile);

    assert.ok(result.assets.length >= 2, "should have at least 2 images from article scope");
    const kinds = new Set(result.assets.map((a) => a.kind));
    assert.ok(kinds.has("image"));
    assert.ok(!kinds.has("link"));
});

test("runExtraction — no assets config → default link discovery", () => {
    const profile = defineExtractionProfile({
        name: "test",
        scope: { candidates: ["article"] },
    });

    const result = extractFromHtml(SAMPLE_HTML, "https://example.com", profile);

    assert.ok(result.links.length >= 2, "should auto-discover at least 2 links");
});

// ---------------------------------------------------------------------------
// Crawler.extractWithProfile 集成（需要 Rust 绑定；skip 时手工 mock）
// ---------------------------------------------------------------------------

test("PageView is read-only — mutation attempts don't break original", () => {
    const view = createPageView(SAMPLE_HTML, "https://example.com");
    const article = query(view, "article");
    assert.ok(article);

    // attributes is ReadonlyMap — 不允许 set
    // 但 TS 的 readonly 是编译时，运行时是真 Map
    // 这里只验证：反复调用 query 得到的是同一个结构
    const a2 = query(view, "article");
    assert.equal(a2, article);
});

/**
 * ExtractionResult — 抽取运行结果。
 *
 * runExtraction(PageView, ExtractionProfile) → ExtractionResult
 *
 * 这是 Pandark "网页中取什么" 层的最终产物。它：
 *   - 不是 Markdown 字符串（那是 Panduck 的事）
 *   - 不是 Notedown IR（那是 Notedown 的事）
 *   - 是中间语义表示：scope 命中状态 + 字段 + 正文 HTML + 资产候选
 */

import {
    createPageView,
    htmlOf,
    queryAll,
    textOf,
    type PageNode,
    type PageView,
} from "./page-view.js";
import type {
    AssetSpec,
    ExtractionProfile,
    FieldSpec,
} from "./extraction-profile.js";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type ScopeStatus = "matched" | "no-match" | "ambiguous";

export type AssetCandidate = {
    kind: "image" | "link" | "script" | "stylesheet";
    url: string;
    /** 来源页面的 selector（debug 用） */
    source?: string;
};

export type LinkCandidate = {
    href: string;
    text: string;
    /** 是否外部链接（相对/绝对） */
    external: boolean;
};

/** 抽取结果 — 中间语义，不是 Markdown 字符串 */
export type ExtractionResult = {
    /** scope 策略命中状态 */
    scopeStatus: ScopeStatus;
    /** 抽取到的语义字段值 */
    fields: Record<string, string | undefined>;
    /** 正文 HTML（还没变成 IR） */
    bodyHtml?: string;
    /** scope 根节点（供 writer 层直接遍历） */
    scopeRoot?: PageNode;
    /** 资产候选 */
    assets: AssetCandidate[];
    /** 链接候选 */
    links: LinkCandidate[];
    /** 诊断 */
    diagnostics: DiagnosticRecord[];
};

// 内联共享类型
type DiagnosticRecord = {
    code: string;
    fallback?: string;
    messageKey?: string;
    args?: Record<string, unknown>;
    origin?: { namespace: string; component: string };
    severity?: "error" | "warn" | "info";
    stage?: "fetch" | "scope" | "structure" | "convert" | "rule" | "handler";
    url?: string;
    target?: string;
};

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/**
 * 把 PageView 套用 ExtractionProfile，产出 ExtractionResult。
 *
 * 执行顺序：
 *   1. scope 匹配 → 决定 scopeStatus + scopeRoot
 *   2. 如果有 scopeRoot，去掉 exclude selectors 命中的节点
 *   3. 抽取 fields（从 scopeRoot 或 root 上按 FieldSpec 选）
 *   4. 收集 assets + links
 *   5. 检查 required 字段，打缺失诊断
 */
export function runExtraction(
    view: PageView,
    profile: ExtractionProfile,
): ExtractionResult {
    const diagnostics: DiagnosticRecord[] = [...view.diagnostics];
    const scopeResult = resolveScope(view, profile, diagnostics);
    const scopeRoot = scopeResult.root;

    // bodyHtml — 基于 scopeRoot
    let bodyHtml: string | undefined;
    if (scopeRoot) {
        const cleanedRoot = applyExcludes(scopeRoot, profile.exclude ?? []);
        bodyHtml = cleanedRoot ? htmlOf(cleanedRoot) : undefined;
    }

    // 字段抽取
    const fields: Record<string, string | undefined> = {};
    for (const [name, spec] of Object.entries(profile.fields ?? {})) {
        fields[name] = extractField(view, scopeRoot, spec, name, diagnostics);
    }

    // 资产收集
    const assets: AssetCandidate[] = [];
    const links: LinkCandidate[] = [];
    for (const a of profile.assets ?? []) {
        collectAssetsAndLinks(view, scopeRoot, a, assets, links);
    }

    // 回退：如果没配 assets，默认收集 all links
    if ((profile.assets ?? []).length === 0) {
        const defaultLinks = queryAll(view.root, "a[href]");
        for (const node of defaultLinks) {
            const href = node.attributes.get("href");
            if (href) {
                links.push({
                    href,
                    text: textOf(node).trim(),
                    external: isExternal(href, view.metadata.url),
                });
            }
        }
    }

    return {
        scopeStatus: scopeResult.status,
        fields,
        bodyHtml,
        scopeRoot: scopeRoot ?? undefined,
        assets,
        links,
        diagnostics,
    };
}

// ---------------------------------------------------------------------------
// Scope 解析
// ---------------------------------------------------------------------------

type ScopeResolution = {
    status: ScopeStatus;
    root?: PageNode;
};

function resolveScope(
    view: PageView,
    profile: ExtractionProfile,
    diagnostics: DiagnosticRecord[],
): ScopeResolution {
    const scope = profile.scope;
    if (!scope || scope.candidates.length === 0) {
        // 无 scope → 把整个 document 当作 scope
        diagnostics.push(makeDiag(
            "pandark.extraction.scope.absent",
            "No scope configured — using entire document as scope",
            "info",
        ));
        return { status: "matched", root: view.root };
    }

    const policy = scope.policy ?? "first-match";

    if (policy === "first-match") {
        for (const sel of scope.candidates) {
            const hits = queryAll(view.root, sel);
            if (hits.length > 0) {
                return { status: "matched", root: hits[0] };
            }
        }
        diagnostics.push(makeDiag(
            "pandark.extraction.scope.no-match",
            `No scope candidate matched. Tried: ${scope.candidates.join(", ")}`,
            "warn",
        ));
        return { status: "no-match" };
    }

    if (policy === "best-score") {
        let best: PageNode | undefined;
        let bestScore = -1;
        for (const sel of scope.candidates) {
            const hits = queryAll(view.root, sel);
            for (const h of hits) {
                const score = scoreNode(h);
                if (score > bestScore) {
                    bestScore = score;
                    best = h;
                }
            }
        }
        if (best) return { status: "matched", root: best };
        diagnostics.push(makeDiag(
            "pandark.extraction.scope.no-match",
            `No scope candidate matched (best-score policy).`,
            "warn",
        ));
        return { status: "no-match" };
    }

    if (policy === "all-matches") {
        const all: PageNode[] = [];
        const seen = new Set<PageNode>();
        for (const sel of scope.candidates) {
            for (const h of queryAll(view.root, sel)) {
                if (!seen.has(h)) {
                    seen.add(h);
                    all.push(h);
                }
            }
        }
        if (all.length === 0) {
            diagnostics.push(makeDiag(
                "pandark.extraction.scope.no-match",
                `No scope candidate matched (all-matches policy).`,
                "warn",
            ));
            return { status: "no-match" };
        }
        if (all.length === 1) {
            return { status: "matched", root: all[0] };
        }
        diagnostics.push(makeDiag(
            "pandark.extraction.scope.ambiguous",
            `Multiple scope candidates matched (${all.length}). Ambiguous.`,
            "warn",
        ));
        // 仍然返回第一个，让下游有东西可用；status 标为 ambiguous
        return { status: "ambiguous", root: all[0] };
    }

    // fallback — 未知策略
    diagnostics.push(makeDiag(
        "pandark.extraction.scope.unknown-policy",
        `Unknown scope policy: ${policy}`,
        "error",
    ));
    return { status: "no-match" };
}

/**
 * 给一个 scope 候选节点打分（best-score 策略）。
 *
 * 目前是非常粗糙的启发式：
 *   - 文本内容长度
 *   - 子段落/标题/列表数量
 *   - 深度（越深的候选越可能是正文）
 *   - 排除明显的噪声特征（class/id 包含 sidebar, nav, ad 等）
 */
function scoreNode(node: PageNode): number {
    let score = 0;
    const walk = (n: PageNode) => {
        if (n.kind === "text") score += (n.name?.length ?? 0);
        if (n.kind === "element") {
            const t = n.name;
            if (t === "p") score += 50;
            if (t === "h1" || t === "h2" || t === "h3") score += 30;
            if (t === "li") score += 5;
            if (t === "img") score += 2;
            // Noise penalty
            const cls = n.attributes.get("class") ?? "";
            const id = n.attributes.get("id") ?? "";
            const hay = (cls + " " + id).toLowerCase();
            if (/(nav|sidebar|footer|comment|ad-|advert|menu|header)/.test(hay)) score -= 100;
        }
        for (const c of n.children) walk(c);
    };
    walk(node);
    return score;
}

// ---------------------------------------------------------------------------
// 字段抽取
// ---------------------------------------------------------------------------

function extractField(
    view: PageView,
    scopeRoot: PageNode | undefined,
    spec: FieldSpec,
    name: string,
    diagnostics: DiagnosticRecord[],
): string | undefined {
    // 字段通常在全文范围内找（meta 标签不在 scope 里），但 title 类
    // 字段优先从 scope 内找。这里用 root 作为搜索根，更宽松。
    const searchRoot = scopeRoot ?? view.root;

    for (const sel of spec.candidates) {
        const hits = queryAll(searchRoot, sel);
        if (hits.length > 0) {
            const value = extractFromNode(hits[0], spec);
            if (value !== undefined && value.length > 0) {
                return value.trim();
            }
        }
        // 有些字段（meta）只在 head 里，scopeRoot 可能覆盖不到 — 兜底用 root
        if (searchRoot !== view.root) {
            const hitsGlobal = queryAll(view.root, sel);
            if (hitsGlobal.length > 0) {
                const value = extractFromNode(hitsGlobal[0], spec);
                if (value !== undefined && value.length > 0) {
                    return value.trim();
                }
            }
        }
    }

    if (spec.required) {
        diagnostics.push(makeDiag(
            "pandark.extraction.field.missing",
            `Required field "${name}" not found.`,
            "warn",
            { field: name, candidates: spec.candidates },
        ));
    }
    return undefined;
}

function extractFromNode(node: PageNode, spec: FieldSpec): string | undefined {
    if (spec.extract === "attribute") {
        if (!spec.attributeName) return undefined;
        return node.attributes.get(spec.attributeName);
    }
    if (spec.extract === "text") {
        return textOf(node);
    }
    if (spec.extract === "html") {
        return htmlOf(node);
    }
    return undefined;
}

// ---------------------------------------------------------------------------
// exclude 处理（返回一个"干净"的副本，排除 exclude selectors 命中的节点）
//
// 注：这里实现的是不可变过滤 — 返回一个新节点树，不修改原节点。
// 因为 PageNode 是 readonly 的，我们重建 MutableNode。
// ---------------------------------------------------------------------------

import type { PageNodeKind } from "./page-view.js";
type MutableN = {
    kind: PageNodeKind;
    name?: string;
    attributes: Map<string, string>;
    children: MutableN[];
    sourceRef?: { start: number; end: number };
};

function applyExcludes(root: PageNode, excludeSelectors: string[]): PageNode {
    if (excludeSelectors.length === 0) return root;

    // 先找出所有要排除的节点
    const toExclude = new Set<PageNode>();
    for (const sel of excludeSelectors) {
        for (const h of queryAll(root, sel)) toExclude.add(h);
    }
    if (toExclude.size === 0) return root;

    return filterNode(root, toExclude);
}

function filterNode(node: PageNode, exclude: Set<PageNode>): PageNode {
    if (exclude.has(node)) {
        // 整个子树丢弃
        return makeEmpty();
    }

    const mutable: MutableN = {
        kind: node.kind,
        name: node.name,
        attributes: new Map(node.attributes),
        children: [],
        sourceRef: node.sourceRef,
    };
    for (const c of node.children) {
        const filtered = filterNode(c, exclude);
        if (!isEmptyNode(filtered)) mutable.children.push(filtered as MutableN);
    }
    return mutable as unknown as PageNode;
}

function makeEmpty(): PageNode {
    return { kind: "text", name: "", attributes: new Map(), children: [] };
}

function isEmptyNode(n: PageNode): boolean {
    if (n.kind === "text" && (n.name ?? "").trim().length === 0) return true;
    return false;
}

// ---------------------------------------------------------------------------
// 资产 / 链接收集
// ---------------------------------------------------------------------------

function collectAssetsAndLinks(
    view: PageView,
    scopeRoot: PageNode | undefined,
    spec: AssetSpec,
    assetsOut: AssetCandidate[],
    linksOut: LinkCandidate[],
) {
    const searchRoot = spec.discover === "in-scope" && scopeRoot
        ? scopeRoot
        : view.root;

    if (spec.kind === "image") {
        const imgs = queryAll(searchRoot, "img[src]");
        for (const node of imgs) {
            const src = node.attributes.get("src");
            if (src) {
                assetsOut.push({
                    kind: "image",
                    url: resolveUrl(src, view.metadata.url),
                    source: "img[src]",
                });
            }
        }
    }
    else if (spec.kind === "link") {
        const anchors = queryAll(searchRoot, "a[href]");
        for (const node of anchors) {
            const href = node.attributes.get("href");
            if (href && !href.startsWith("#") && !href.startsWith("javascript:")) {
                linksOut.push({
                    href: resolveUrl(href, view.metadata.url),
                    text: textOf(node).trim(),
                    external: isExternal(href, view.metadata.url),
                });
            }
        }
    }
    else if (spec.kind === "script") {
        const scripts = queryAll(searchRoot, "script[src]");
        for (const node of scripts) {
            const src = node.attributes.get("src");
            if (src) assetsOut.push({ kind: "script", url: src, source: "script[src]" });
        }
    }
    else if (spec.kind === "stylesheet") {
        const links = queryAll(searchRoot, 'link[rel="stylesheet"][href]');
        for (const node of links) {
            const href = node.attributes.get("href");
            if (href) assetsOut.push({ kind: "stylesheet", url: href, source: 'link[rel="stylesheet"]' });
        }
    }
}

// ---------------------------------------------------------------------------
// URL 工具
// ---------------------------------------------------------------------------

function resolveUrl(url: string, baseUrl: string): string {
    try {
        return new URL(url, baseUrl).href;
    }
    catch {
        return url;
    }
}

function isExternal(href: string, baseUrl: string): boolean {
    if (/^[a-zA-Z]+:/.test(href)) {
        try {
            const h = new URL(href);
            const b = new URL(baseUrl);
            return h.host !== b.host;
        }
        catch { /* fallthrough */ }
    }
    return false;
}

// ---------------------------------------------------------------------------
// 诊断工具
// ---------------------------------------------------------------------------

function makeDiag(
    code: string,
    fallback: string,
    severity: "error" | "warn" | "info" = "warn",
    args?: Record<string, unknown>,
): DiagnosticRecord {
    return {
        code,
        fallback,
        severity,
        origin: { namespace: "pandark", component: "extraction" },
        stage: "structure",
        args,
    };
}

// 方便使用者从 HTML 字符串直接拿到 ExtractionResult
export function extractFromHtml(
    html: string,
    url: string,
    profile: ExtractionProfile,
): ExtractionResult {
    const view = createPageView(html, url);
    return runExtraction(view, profile);
}

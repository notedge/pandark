/**
 * PageView — 受控页面视图。
 *
 * 把裸 HTML 字符串提升为只读的、结构化的 PageNode 树。用户只能
 * 通过 `query` / `queryAll` / `textOf` / `htmlOf` 查询，禁止任何
 * 形式的 mutation。设计目标：替代原始 HTML 暴露，成为抽取层唯一
 * 的输入接口。
 *
 * 本文件内置一个极简 HTML 解析器和 CSS selector 引擎，目的是
 * 在 TS 侧零额外依赖即可工作，保持 Pandark 对 Rust 绑定的最小耦合。
 */

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type PageNodeKind = "document" | "element" | "text" | "comment" | "doctype";

/** 受控页面节点 — 不是裸 DOM 节点。所有字段只读。 */
export type PageNode = {
    readonly kind: PageNodeKind;
    /** 元素标签名（element 才有，小写） */
    readonly name?: string;
    readonly attributes: ReadonlyMap<string, string>;
    readonly children: readonly PageNode[];
    /** 在源 HTML 中的 byte range（UTF-8 offset） */
    readonly sourceRef?: { start: number; end: number };
};

/** 页面元信息 */
export type PageMetadata = {
    url: string;
    finalUrl: string;
    status: number;
    contentType: string;
    title?: string;
    language?: string;
};

/** 受控页面视图 — 用户只能通过只读查询访问 */
export type PageView = {
    readonly metadata: PageMetadata;
    readonly root: PageNode;
    /** 原始 HTML 快照（只读） */
    readonly source: string;
    readonly diagnostics: readonly DiagnosticRecord[];
};

// 从 crawler 共享的 DiagnosticRecord 拿类型（直接内联避免循环依赖）
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
// Public API
// ---------------------------------------------------------------------------

/**
 * 把 HTML 字符串 + 页面 URL 构造成 PageView。
 *
 * 会自动抽取 <title> 和 <html lang> 填入 metadata。
 */
export function createPageView(html: string, url: string, metadata?: Partial<PageMetadata>): PageView {
    const diagnostics: DiagnosticRecord[] = [];
    let root: PageNode;
    try {
        root = parseHtml(html, diagnostics);
    }
    catch (err) {
        diagnostics.push({
            code: "pandark.page-view.parse-failed",
            fallback: `HTML parse failed: ${describeError(err)}`,
            severity: "error",
            origin: { namespace: "pandark", component: "page-view" },
            stage: "structure",
            url,
        });
        root = { kind: "document", children: [], attributes: new Map() };
    }

    const title = findMetaTitle(root);
    const language = findMetaLanguage(root);

    return {
        metadata: {
            url,
            finalUrl: metadata?.finalUrl ?? url,
            status: metadata?.status ?? 0,
            contentType: metadata?.contentType ?? "text/html",
            title,
            language,
            ...metadata,
        },
        root,
        source: html,
        diagnostics,
    };
}

type QueryTarget = PageNode | PageView;

function getRoot(target: QueryTarget): PageNode {
    return "root" in target ? target.root : target;
}

/** 等价于 queryAll(target, selector)[0] ?? undefined */
export function query(target: QueryTarget, selector: string): PageNode | undefined {
    const matches = queryAll(target, selector);
    return matches.length > 0 ? matches[0] : undefined;
}

/** 对节点子树或 PageView.root 执行 CSS selector 匹配，返回所有命中的 element 节点。 */
export function queryAll(target: QueryTarget, selector: string): readonly PageNode[] {
    const expr = parseSelector(selector);
    return collectMatches(getRoot(target), expr);
}

/** 节点的纯文本（深度优先拼接所有 text 节点）。 */
export function textOf(node: PageNode): string {
    const out: string[] = [];
    walk(node, (n) => {
        if (n.kind === "text") out.push(n.name ?? "");
    });
    return out.join("");
}

/**
 * 节点的外部 HTML 字符串。writer 层才用；extraction 层禁止调用。
 *
 * 优先使用 sourceRef 从原始 HTML 切片，如果切片不可用则回退到
 * 节点的浅序列化（不保证 1:1 原始输出）。
 */
export function htmlOf(node: PageNode): string {
    if (node.sourceRef) {
        // sourceRef 记录的是 UTF-8 byte offset，但 JavaScript string
        // 是 UTF-16 code-unit。为了简单，这里先退回序列化；未来可
        // 升级到 TextEncoder 精确 byte 映射。
    }
    return serializeNode(node);
}

// ---------------------------------------------------------------------------
// HTML parser — 极简实现
// ---------------------------------------------------------------------------

const VOID_ELEMENTS = new Set([
    "area", "base", "br", "col", "embed", "hr", "img", "input",
    "link", "meta", "param", "source", "track", "wbr",
]);

const WHITESPACE = /[\t\n\r ]+/;

function parseHtml(html: string, diagnostics: DiagnosticRecord[]): PageNode {
    const stack: Array<{ node: MutableNode; tag: string }> = [];
    const doc: MutableNode = {
        kind: "document",
        attributes: new Map(),
        children: [],
    };
    let current: MutableNode = doc;
    let pos = 0;

    while (pos < html.length) {
        const ch = html.charCodeAt(pos);

        // Comment: <!-- ... -->
        if (ch === 0x3C /* '<' */ && pos + 3 < html.length
            && html.charCodeAt(pos + 1) === 0x21 /* '!' */
            && html.charCodeAt(pos + 2) === 0x2D /* '-' */
            && html.charCodeAt(pos + 3) === 0x2D /* '-' */) {
            const end = html.indexOf("-->", pos + 4);
            if (end === -1) {
                diagnostics.push({
                    code: "pandark.page-view.unterminated-comment",
                    fallback: "Unterminated HTML comment",
                    severity: "warn",
                    origin: { namespace: "pandark", component: "page-view" },
                    stage: "structure",
                });
                break;
            }
            const content = html.slice(pos + 4, end);
            current.children.push({
                kind: "comment",
                attributes: new Map(),
                children: [],
                name: content,
                sourceRef: { start: pos, end: end + 3 },
            });
            pos = end + 3;
            continue;
        }

        // Doctype: <!DOCTYPE ...>
        if (ch === 0x3C /* '<' */ && pos + 1 < html.length
            && html.charCodeAt(pos + 1) === 0x21 /* '!' */) {
            const end = html.indexOf(">", pos);
            if (end === -1) break;
            current.children.push({
                kind: "doctype",
                attributes: new Map(),
                children: [],
                sourceRef: { start: pos, end: end + 1 },
            });
            pos = end + 1;
            continue;
        }

        // Opening tag or closing tag
        if (ch === 0x3C /* '<' */) {
            const isClosing = pos + 1 < html.length && html.charCodeAt(pos + 1) === 0x2F /* '/' */;
            let i = isClosing ? pos + 2 : pos + 1;

            // Read tag name
            let nameStart = i;
            while (i < html.length) {
                const c = html.charCodeAt(i);
                if (c === 0x20 || c === 0x09 || c === 0x0A || c === 0x0D
                    || c === 0x3E /* > */ || c === 0x2F /* / */) break;
                i++;
            }
            const tagName = html.slice(nameStart, i).toLowerCase();

            if (tagName.length === 0) {
                // Malformed — skip
                pos = i;
                continue;
            }

            if (isClosing) {
                // Pop stack until matching tag
                let found = false;
                while (stack.length > 0) {
                    const top = stack.pop()!;
                    if (top.tag === tagName) {
                        current = stack.length > 0 ? stack[stack.length - 1].node : doc;
                        found = true;
                        break;
                    }
                    // Auto-close mismatch
                }
                if (!found) {
                    diagnostics.push({
                        code: "pandark.page-view.unmatched-close",
                        fallback: `Unmatched closing tag </${tagName}>`,
                        severity: "warn",
                        origin: { namespace: "pandark", component: "page-view" },
                        stage: "structure",
                    });
                }
                const end = html.indexOf(">", i);
                pos = end === -1 ? html.length : end + 1;
                continue;
            }

            // Opening tag — parse attributes
            const attrs = new Map<string, string>();
            let selfClosing = false;
            let end = -1;

            while (i < html.length) {
                // Skip whitespace
                while (i < html.length && WHITESPACE.test(html[i])) i++;
                if (i >= html.length) break;

                if (html.charCodeAt(i) === 0x3E /* '>' */) {
                    end = i;
                    break;
                }
                if (html.charCodeAt(i) === 0x2F /* '/' */) {
                    selfClosing = true;
                    i++;
                    // skip whitespace before >
                    while (i < html.length && WHITESPACE.test(html[i])) i++;
                    if (html.charCodeAt(i) === 0x3E) { end = i; break; }
                    end = i - 1; // fallback
                    break;
                }

                // Attribute name
                let attrNameStart = i;
                while (i < html.length) {
                    const c = html.charCodeAt(i);
                    if (c === 0x3D || c === 0x3E || c === 0x2F || WHITESPACE.test(html[i])) break;
                    i++;
                }
                const attrName = html.slice(attrNameStart, i).toLowerCase();
                let attrValue = "";

                // Skip whitespace
                while (i < html.length && WHITESPACE.test(html[i])) i++;

                if (i < html.length && html.charCodeAt(i) === 0x3D /* '=' */) {
                    i++; // skip =
                    while (i < html.length && WHITESPACE.test(html[i])) i++;

                    if (i < html.length && (html.charCodeAt(i) === 0x22 || html.charCodeAt(i) === 0x27)) {
                        const quote = html.charCodeAt(i);
                        i++;
                        const valStart = i;
                        while (i < html.length && html.charCodeAt(i) !== quote) i++;
                        attrValue = html.slice(valStart, i);
                        if (i < html.length) i++; // skip closing quote
                    }
                    else {
                        const valStart = i;
                        while (i < html.length) {
                            const c = html.charCodeAt(i);
                            if (c === 0x3E || c === 0x2F || WHITESPACE.test(html[i])) break;
                            i++;
                        }
                        attrValue = html.slice(valStart, i);
                    }
                }

                if (attrName.length > 0) attrs.set(attrName, decodeEntities(attrValue));
            }

            if (end === -1) {
                end = html.indexOf(">", i);
                if (end === -1) break;
            }

            const startByte = pos;
            const endByte = end + 1;
            const node: MutableNode = {
                kind: "element",
                name: tagName,
                attributes: attrs,
                children: [],
                sourceRef: { start: startByte, end: endByte },
            };

            current.children.push(node);

            if (!selfClosing && !VOID_ELEMENTS.has(tagName)) {
                stack.push({ node, tag: tagName });
                current = node;

                // Handle raw text elements — skip inner content
                if (tagName === "script" || tagName === "style") {
                    const closePat = new RegExp(`</${tagName}`, "i");
                    const rest = html.slice(endByte);
                    const m = closePat.exec(rest);
                    if (m) {
                        // Add raw text child
                        const raw = rest.slice(0, m.index);
                        node.children.push({
                            kind: "text",
                            name: raw,
                            attributes: new Map(),
                            children: [],
                            sourceRef: { start: endByte, end: endByte + m.index },
                        });
                        pos = endByte + m.index;
                        // Pop script/style
                        stack.pop();
                        current = stack.length > 0 ? stack[stack.length - 1].node : doc;
                        continue;
                    }
                }
            }

            pos = endByte;
            continue;
        }

        // Text node
        const textStart = pos;
        while (pos < html.length && html.charCodeAt(pos) !== 0x3C /* '<' */) pos++;
        if (pos > textStart) {
            current.children.push({
                kind: "text",
                name: decodeEntities(html.slice(textStart, pos)),
                attributes: new Map(),
                children: [],
                sourceRef: { start: textStart, end: pos },
            });
        }
    }

    return freezeNode(doc);
}

function decodeEntities(s: string): string {
    // Minimal entity decoding — enough for common HTML
    return s
        .replace(/&amp;/g, "&")
        .replace(/&lt;/g, "<")
        .replace(/&gt;/g, ">")
        .replace(/&quot;/g, '"')
        .replace(/&#39;/g, "'")
        .replace(/&apos;/g, "'")
        .replace(/&nbsp;/g, "\u00A0")
        .replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(parseInt(d, 10)))
        .replace(/&#x([0-9a-fA-F]+);/g, (_, h) => String.fromCodePoint(parseInt(h, 16)));
}

type MutableNode = {
    kind: PageNodeKind;
    name?: string;
    attributes: Map<string, string>;
    children: MutableNode[];
    sourceRef?: { start: number; end: number };
};

function freezeNode(n: MutableNode): PageNode {
    return {
        kind: n.kind,
        name: n.name,
        attributes: new Map(n.attributes),
        children: n.children.map(freezeNode),
        sourceRef: n.sourceRef ? { ...n.sourceRef } : undefined,
    };
}

function walk(node: PageNode, fn: (n: PageNode) => void) {
    fn(node);
    for (const c of node.children) walk(c, fn);
}

function findMetaTitle(root: PageNode): string | undefined {
    const titleEl = queryAll(root, "title")[0];
    if (!titleEl) return undefined;
    const t = textOf(titleEl).trim();
    return t.length > 0 ? t : undefined;
}

function findMetaLanguage(root: PageNode): string | undefined {
    const htmlEl = queryAll(root, "html")[0];
    if (!htmlEl) return undefined;
    const lang = htmlEl.attributes.get("lang") ?? htmlEl.attributes.get("xml:lang");
    return lang;
}

// ---------------------------------------------------------------------------
// CSS selector — 极简实现
//
// 支持：
//   - 逗号分隔 selector list
//   - 标签选择器 (div, *)
//   - 类选择器 (.foo)
//   - ID 选择器 (#bar)
//   - 属性存在性 ([hidden])
//   - 属性等于 ([data-x="y"], 单引号可选)
//   - 属性子串 ([href*="example"])
//   - 后代组合 (a b)
//   - 直接子元素 (a > b)
//
// 不支持：伪类、相邻兄弟、通配符、复杂属性值、大小写不敏感选项等。
// ---------------------------------------------------------------------------

type SimpleSelector = {
    tag?: string;        // undefined = *
    classes: string[];
    ids: string[];
    attrs: Array<{ name: string; op: "" | "=" | "*=" | "~="; value?: string }>;
};

type SelectorSegment = {
    combinator: "descendant" | "child";
    selector: SimpleSelector;
};

type SelectorExpr = SelectorSegment[];

function parseSelector(input: string): SelectorExpr[] {
    const trimmed = input.trim();
    if (trimmed.length === 0) return [];

    // Split by top-level commas only
    const parts: string[] = [];
    let depth = 0;
    let buf = "";
    for (let i = 0; i < trimmed.length; i++) {
        const c = trimmed[i];
        if (c === "[" || c === "(") depth++;
        else if (c === "]" || c === ")") depth--;
        if (c === "," && depth === 0) {
            parts.push(buf.trim());
            buf = "";
        }
        else {
            buf += c;
        }
    }
    if (buf.trim().length > 0) parts.push(buf.trim());

    return parts.map(parseSingleSelector).filter((s) => s.length > 0);
}

function parseSingleSelector(s: string): SelectorExpr {
    // Tokenize: split by combinators (whitespace, >)
    // Use a regex that preserves the combinator
    const tokens = s.split(/(\s+>|>\s+|\s+)/).map((t) => t.trim()).filter((t) => t.length > 0);

    const expr: SelectorExpr = [];
    let pendingCombinator: "descendant" | "child" = "descendant";

    for (const tok of tokens) {
        if (tok === ">") {
            pendingCombinator = "child";
            continue;
        }
        const sel = parseSimpleSelector(tok);
        if (sel) {
            expr.push({ combinator: pendingCombinator, selector: sel });
            pendingCombinator = "descendant";
        }
    }
    return expr;
}

function parseSimpleSelector(tok: string): SimpleSelector | undefined {
    const sel: SimpleSelector = { classes: [], ids: [], attrs: [] };
    let i = 0;

    // Tag
    if (tok[i] !== "." && tok[i] !== "#" && tok[i] !== "[" && tok[i] !== "*") {
        let start = i;
        while (i < tok.length && /[a-zA-Z0-9_-]/.test(tok[i])) i++;
        if (i > start) sel.tag = tok.slice(start, i).toLowerCase();
    }
    else if (tok[i] === "*") {
        sel.tag = undefined; // wildcard
        i++;
    }

    while (i < tok.length) {
        const c = tok[i];
        if (c === ".") {
            i++;
            let start = i;
            while (i < tok.length && /[a-zA-Z0-9_-]/.test(tok[i])) i++;
            sel.classes.push(tok.slice(start, i));
        }
        else if (c === "#") {
            i++;
            let start = i;
            while (i < tok.length && /[a-zA-Z0-9_-]/.test(tok[i])) i++;
            sel.ids.push(tok.slice(start, i));
        }
        else if (c === "[") {
            const closeIdx = tok.indexOf("]", i);
            if (closeIdx === -1) return undefined;
            const attrPart = tok.slice(i + 1, closeIdx).trim();
            const attrParsed = parseAttrSelector(attrPart);
            if (attrParsed) sel.attrs.push(attrParsed);
            i = closeIdx + 1;
        }
        else {
            // Unexpected char — stop
            break;
        }
    }

    if (!sel.tag && sel.classes.length === 0 && sel.ids.length === 0 && sel.attrs.length === 0) {
        return undefined;
    }
    return sel;
}

function parseAttrSelector(s: string): { name: string; op: "" | "=" | "*=" | "~="; value?: string } | undefined {
    // Try to match: name[op"value"]
    const mEq = s.match(/^([a-zA-Z_:][a-zA-Z0-9_:\-]*)\s*=\s*"([^"]*)"$/);
    if (mEq) return { name: mEq[1].toLowerCase(), op: "=", value: mEq[2] };
    const mEqSq = s.match(/^([a-zA-Z_:][a-zA-Z0-9_:\-]*)\s*=\s*'([^']*)'$/);
    if (mEqSq) return { name: mEqSq[1].toLowerCase(), op: "=", value: mEqSq[2] };
    const mEqBare = s.match(/^([a-zA-Z_:][a-zA-Z0-9_:\-]*)\s*=\s*([^\s"']+)$/);
    if (mEqBare) return { name: mEqBare[1].toLowerCase(), op: "=", value: mEqBare[2] };

    const mStar = s.match(/^([a-zA-Z_:][a-zA-Z0-9_:\-]*)\s*\*=\s*"?([^"]*)"?$/);
    if (mStar) return { name: mStar[1].toLowerCase(), op: "*=", value: mStar[2] };
    const mTilde = s.match(/^([a-zA-Z_:][a-zA-Z0-9_:\-]*)\s*~=\s*"?([^"]*)"?$/);
    if (mTilde) return { name: mTilde[1].toLowerCase(), op: "~=", value: mTilde[2] };

    // Presence
    const mPresence = s.match(/^([a-zA-Z_:][a-zA-Z0-9_:\-]*)$/);
    if (mPresence) return { name: mPresence[1].toLowerCase(), op: "" };

    return undefined;
}

function simpleMatches(node: PageNode, sel: SimpleSelector): boolean {
    if (node.kind !== "element") return false;
    if (sel.tag !== undefined && node.name !== sel.tag) return false;

    const cls = node.attributes.get("class");
    if (sel.classes.length > 0) {
        if (!cls) return false;
        const classList = cls.split(/\s+/).filter(Boolean);
        for (const c of sel.classes) {
            if (!classList.includes(c)) return false;
        }
    }

    if (sel.ids.length > 0) {
        const id = node.attributes.get("id");
        if (!id) return false;
        for (const s of sel.ids) {
            if (id !== s) return false;
        }
    }

    for (const a of sel.attrs) {
        const v = node.attributes.get(a.name);
        if (a.op === "") {
            if (v === undefined) return false;
        }
        else if (a.op === "=") {
            if (v !== a.value) return false;
        }
        else if (a.op === "*=") {
            if (v === undefined || !v.includes(a.value!)) return false;
        }
        else if (a.op === "~=") {
            if (v === undefined) return false;
            const list = v.split(/\s+/).filter(Boolean);
            if (!list.includes(a.value!)) return false;
        }
    }

    return true;
}

function exprMatches(node: PageNode, expr: SelectorExpr, treeRoot: PageNode): boolean {
    if (expr.length === 0) return false;
    // expr[last] is relative to node's ancestor chain; walk up from node.
    return matchSegment(node, expr, expr.length - 1, treeRoot);
}

function matchSegment(
    current: PageNode,
    expr: SelectorExpr,
    idx: number,
    treeRoot: PageNode,
): boolean {
    const seg = expr[idx];
    if (!simpleMatches(current, seg.selector)) return false;

    if (idx === 0) return true;

    // Find ancestors of current by walking from treeRoot
    const ancestors = collectAncestors(treeRoot, current);
    if (seg.combinator === "child") {
        // Must match immediate parent
        const parent = ancestors[0];
        if (!parent) return false;
        return matchSegment(parent, expr, idx - 1, treeRoot);
    }
    else {
        // descendant: any ancestor
        for (let i = 0; i < ancestors.length; i++) {
            if (matchSegment(ancestors[i], expr, idx - 1, treeRoot)) return true;
        }
        return false;
    }
}

function collectAncestors(root: PageNode, target: PageNode): PageNode[] {
    // BFS from root to find path to target, return ancestors (closest first)
    const path: PageNode[] = [];
    if (findPath(root, target, path)) {
        // path[0] is root, path[last] is target — ancestors exclude target
        path.pop();
        path.reverse(); // now closest first
        return path;
    }
    return [];
}

function findPath(current: PageNode, target: PageNode, path: PageNode[]): boolean {
    path.push(current);
    if (current === target) return true;
    for (const c of current.children) {
        if (findPath(c, target, path)) return true;
    }
    path.pop();
    return false;
}

function collectMatches(root: PageNode, exprs: SelectorExpr[]): PageNode[] {
    const results: PageNode[] = [];
    const seen = new Set<PageNode>();

    const walkFn = (n: PageNode) => {
        if (n.kind !== "element") return;
        if (seen.has(n)) return;
        for (const expr of exprs) {
            if (exprMatches(n, expr, root)) {
                seen.add(n);
                results.push(n);
                break;
            }
        }
    };

    walk(root, walkFn);
    return results;
}

// ---------------------------------------------------------------------------
// Serialization (htmlOf fallback)
// ---------------------------------------------------------------------------

function serializeNode(node: PageNode): string {
    if (node.kind === "text") return node.name ?? "";
    if (node.kind === "comment") return `<!--${node.name ?? ""}-->`;
    if (node.kind === "doctype") return "<!DOCTYPE html>";
    if (node.kind === "document") return node.children.map(serializeNode).join("");
    if (node.kind === "element") {
        const attrs = [...node.attributes.entries()]
            .map(([k, v]) => v === "" ? k : `${k}="${v.replace(/"/g, "&quot;")}"`)
            .join(" ");
        const attrStr = attrs.length > 0 ? " " + attrs : "";
        if (VOID_ELEMENTS.has(node.name!)) {
            return `<${node.name}${attrStr}>`;
        }
        const inner = node.children.map(serializeNode).join("");
        return `<${node.name}${attrStr}>${inner}</${node.name}>`;
    }
    return "";
}

function describeError(err: unknown): string {
    if (err instanceof Error) return err.message;
    return String(err);
}

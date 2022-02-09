# @notedge/pandark-skills

Agent skill pack for **people who use Pandark** to extract saved pages or run bounded crawls—not for contributors
changing the Rust crates.

Install once so coding agents know how to install `@notedge/pandark`, pick CLI or Node entry points, interpret crawl
reports, and explain why a run paused or produced partial output.

**Scope:** instructions only. The installer does not ship the native binding, a headless browser, or production-grade
politeness guarantees.

## 📥 Install

```bash
npx @notedge/pandark-skills
```

```bash
npx @notedge/pandark-skills -g
npx @notedge/pandark-skills -a cursor -y
```

`-g` installs the wrapper globally. `-a cursor -y` targets Cursor when the wrapper supports that agent.

## 💬 Example prompts

```text
Use Pandark to inspect ./saved/page.html and extract a semantic document.
Write ./out/page.document.json and ./out/page.report.json and explain any gaps.
```

```text
Plan a crawl from https://example.com/docs with depth 1 and budget 30.
Show the frontier and whether this release is safe to run against production.
```

```text
Crawl ./seeds.txt with budget 100 and browser fixtures in ./fixtures/browser.
If the run pauses, tell me where the checkpoint is and how to resume.
```

## ✅ What the agent checks

- Whether `@notedge/pandark` and the platform binary load (`pandark doctor`)
- Smallest supported workflow for the user's input (extract before site-wide crawl)
- `--depth` and `--budget` on crawl commands
- Report JSON vs `committedPages` when the user expects files on disk
- Browser fixture or endpoint requirements for dynamic pages
- Honest limits: prototype crawler, no bundled login browser, WASM without crawl

## 📎 Full skill

See [`skills/pandark/SKILL.md`](skills/pandark/SKILL.md) for trigger conditions, step-by-step execution, and failure
handling.

Install the runtime separately:

```bash
npm install @notedge/pandark
```

License: MPL-2.0

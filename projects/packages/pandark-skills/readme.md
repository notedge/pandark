# @notedge/pandark-skills

面向 **使用 Pandark 做网页采集与语义抽取** 的终端用户与编码代理技能包，不是给改 Rust 内核的贡献者用的。

安装一次后，编码代理会知道如何安装 `@notedge/pandark`、选用 CLI 或 Node API、处理暂停/恢复与浏览器降级，并用通俗语言解释采集报告与失败原因。

## 安装技能

```bash
npx @notedge/pandark-skills
```

```bash
npx @notedge/pandark-skills -g
npx @notedge/pandark-skills -a cursor -y
```

## 示例提问

```text
用 Pandark 从 https://example.com/docs 抓取两层链接，预算 200 次请求，把 JSON 报告给我。
```

```text
我有一份 seeds.txt，批量 crawl 这些 URL。HTTP 失败时回退到浏览器 fixture 目录。
```

```text
crawl 遇到登录页暂停了，checkpoint 写在哪？怎么用 resume 继续？
```

```text
本地 page.html 怎么抽成 notedown-ir？先 inspect 再看能不能 extract。
```

## 技能覆盖范围

- 安装与 `pandark doctor` 自检
- CLI：`plan` / `fetch` / `crawl` / `resume` / `extract` / `inspect` / `doctor`
- 种子 URL 与换行 seed 文件
- 挑战策略、浏览器降级、checkpoint 暂停与恢复
- Node 脚本接入 `loadPandarkNode()`
- 诚实说明当前版本尚未完成的能力（完整无头浏览器、批量落盘目录等）

完整代理指南见 `skills/pandark/SKILL.md`。

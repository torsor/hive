// node --test test/chat-render.test.mjs
// (from apps/cc-console)
//
// The renderer is the one place agent-authored text becomes markup inside a
// webview that holds `window.__TAURI__`. The injection cases below are the
// point of the file; the markdown cases exist so that fixing one does not
// quietly break the other.

import assert from "node:assert/strict";
import { test } from "node:test";

import { escapeHtml, renderMarkdownWithMath } from "../src/chat-render.js";

// A KaTeX stub: records what it was asked to render, emits an inert marker.
function stubKatex(calls = []) {
  return {
    calls,
    renderToString(body, options) {
      calls.push({ body, options });
      return `<span class="katex" data-display="${!!options.displayMode}">${escapeHtml(body)}</span>`;
    },
  };
}

const render = (text, katex = stubKatex()) => renderMarkdownWithMath(text, katex);

test("script tags never survive", () => {
  const html = render("Look: <script>alert(1)</script> done");
  assert.ok(!html.includes("<script"));
  assert.ok(html.includes("&lt;script&gt;"));
});

test("event-handler attributes never survive", () => {
  const html = render('<img src=x onerror="alert(1)">');
  // The words "onerror=" remain, as text — that is the correct outcome. What
  // must not survive is the tag that would give them somewhere to attach.
  assert.ok(!html.includes("<img"));
  assert.ok(html.includes("&lt;img"));
  assert.ok(html.includes("&quot;"));
});

test("javascript: links render as plain text, not anchors", () => {
  const html = render("[click](javascript:alert(1))");
  assert.ok(!html.includes("<a "));
  assert.ok(html.includes("javascript:alert(1)"));
});

test("data: and vbscript: links are refused too", () => {
  for (const scheme of ["data:text/html;base64,PHNjcmlwdD4=", "vbscript:msgbox"]) {
    assert.ok(!render(`[x](${scheme})`).includes("<a "), scheme);
  }
});

test("http, https and mailto links are allowed", () => {
  for (const url of ["https://arxiv.org/abs/1234", "http://x.test", "mailto:a@b.test"]) {
    const html = render(`[label](${url})`);
    assert.ok(html.includes(`href="${url}"`), url);
    assert.ok(html.includes('rel="noopener noreferrer"'), url);
  }
});

test("html inside a code fence is shown, not executed", () => {
  const html = render("```\n<script>alert(1)</script>\n```");
  assert.ok(html.includes("<pre"));
  assert.ok(!html.includes("<script"));
  assert.ok(html.includes("&lt;script&gt;"));
});

test("katex is never given trust", () => {
  const katex = stubKatex();
  render("$$\\href{javascript:alert(1)}{x}$$", katex);
  assert.equal(katex.calls.length, 1);
  assert.notEqual(katex.calls[0].options.trust, true);
});

test("display math is lifted whole and marked display", () => {
  const katex = stubKatex();
  render("before\n$$\\int_0^1 f(x)\\,dx = 0$$\nafter", katex);
  assert.equal(katex.calls.length, 1);
  assert.equal(katex.calls[0].body.trim(), "\\int_0^1 f(x)\\,dx = 0");
  assert.equal(katex.calls[0].options.displayMode, true);
});

test("inline math is marked inline", () => {
  const katex = stubKatex();
  render("Let $\\pi$ be a uniformizer.", katex);
  assert.equal(katex.calls[0].body, "\\pi");
  assert.equal(katex.calls[0].options.displayMode, false);
});

test("both LaTeX delimiter styles work", () => {
  const katex = stubKatex();
  render("\\(a+b\\) and \\[c+d\\]", katex);
  // Rendered in document order, not extraction order: \(a+b\) is inline and
  // comes first on the line, \[c+d\] is display and comes second.
  assert.deepEqual(katex.calls.map((c) => c.body), ["a+b", "c+d"]);
  assert.deepEqual(katex.calls.map((c) => c.options.displayMode), [false, true]);
});

test("markdown never mangles the inside of math", () => {
  // `a_1 * b_2` would otherwise become emphasis and lose the underscores —
  // the single most common way a maths-unaware renderer corrupts an equation.
  const katex = stubKatex();
  render("$a_1 * b_2 * c_3$", katex);
  assert.equal(katex.calls[0].body, "a_1 * b_2 * c_3");
});

test("a dollar sign inside code is not math", () => {
  const katex = stubKatex();
  const html = render("run `echo $HOME` first", katex);
  assert.equal(katex.calls.length, 0);
  assert.ok(html.includes("<code>echo $HOME</code>"));
});

test("unpaired dollars stay prose", () => {
  const katex = stubKatex();
  render("it cost $5 yesterday\n\nand $6 today", katex);
  assert.equal(katex.calls.length, 0);
});

test("unrenderable math falls back to its source, not a gap", () => {
  const exploding = { renderToString() { throw new Error("nope"); } };
  const html = renderMarkdownWithMath("$\\badmacro{}$", exploding);
  assert.ok(html.includes("math-error"));
  assert.ok(html.includes("\\badmacro"));
});

test("emphasis, strong and inline code", () => {
  const html = render("**bold** and *em* and `code`");
  assert.ok(html.includes("<strong>bold</strong>"));
  assert.ok(html.includes("<em>em</em>"));
  assert.ok(html.includes("<code>code</code>"));
});

test("headings are demoted below the panel's own", () => {
  assert.ok(render("# Title").includes("<h3>Title</h3>"));
  assert.ok(render("## Sub").includes("<h4>Sub</h4>"));
});

test("bulleted and numbered lists", () => {
  const bullets = render("- one\n- two");
  assert.ok(bullets.includes("<ul><li>one</li><li>two</li></ul>"));
  const numbers = render("1. one\n2. two");
  assert.ok(numbers.includes("<ol><li>one</li><li>two</li></ol>"));
});

test("a wrapped list item stays one item", () => {
  const html = render("- a claim that runs\n  onto a second line\n- next");
  assert.ok(html.includes("<li>a claim that runs onto a second line</li>"));
  assert.ok(html.includes("<li>next</li>"));
});

test("blockquote and rule", () => {
  assert.ok(render("> quoted").includes("<blockquote><p>quoted</p></blockquote>"));
  assert.ok(render("---").includes("<hr>"));
});

test("pipe tables become tables", () => {
  const html = render("| a | b |\n|---|---|\n| 1 | 2 |");
  assert.ok(html.includes('<table class="t">'));
  assert.ok(html.includes("<th>a</th>"));
  assert.ok(html.includes("<td>2</td>"));
});

test("paragraphs are separated on blank lines", () => {
  const html = render("first para\n\nsecond para");
  assert.equal((html.match(/<p>/g) || []).length, 2);
});

test("empty and nullish input do not throw", () => {
  for (const value of ["", null, undefined, "   "]) {
    assert.equal(typeof renderMarkdownWithMath(value, stubKatex()), "string");
  }
});

test("a realistic agent message renders end to end", () => {
  const katex = stubKatex();
  const html = render([
    "It splits, but **not canonically**. The residue sequence",
    "$$0 \\to \\mathrm{Br}(\\mathcal{O}) \\to \\mathrm{Br}(K) \\to H^1(k) \\to 0$$",
    "admits a splitting for each choice of uniformizer $\\pi$.",
    "",
    "- the map is independent of $\\pi$ on $H^1$",
    "- the splitting is not",
  ].join("\n"), katex);
  assert.equal(katex.calls.length, 4);
  assert.ok(html.includes("<strong>not canonically</strong>"));
  assert.ok(html.includes("<ul>"));
  assert.ok(!html.includes("\u0000"), "no placeholder sentinel leaked");
});

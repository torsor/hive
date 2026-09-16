// Markdown + LaTeX to HTML, for the rendered chat panel.
//
// Why this exists rather than `marked` + `DOMPurify`. The text being rendered is
// written by an agent and travels through no sanitising layer — the transcript
// adapter passes prose through verbatim, deliberately, because rendering is the
// client's business. This runs inside a Tauri webview where `window.__TAURI__`
// is in scope, so an injected <script> is not a defaced paragraph, it is
// arbitrary invocation of every fleet command the GUI can issue.
//
// So the order here is the security property, not a style choice:
//
//   1. lift fenced code and math OUT, into placeholders
//   2. escape EVERY remaining angle bracket and ampersand
//   3. apply markdown, which is now the only thing that can emit a tag
//   4. put code and math back, code escaped, math through KaTeX
//
// After step 2 there is no attacker-controlled `<` left in the stream, so the
// output can only contain the tags this file writes. That is a structural
// property; it does not depend on a blocklist staying current. `marked` emits
// raw HTML by default and would need a second dependency to undo it.
//
// KaTeX is called with its default `trust: false`, which is what disables
// \href, \htmlClass and friends — the escape hatch that would otherwise put
// author-controlled markup back into the output.

const MATH = "\u0000M";
const CODE = "\u0000C";
const END = "\u0000";

// Anything else — javascript:, data:, vbscript: — renders as plain text.
const SAFE_SCHEME = /^(?:https?:\/\/|mailto:)/i;

export function escapeHtml(text) {
  return String(text)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

// --- step 1: lift out what markdown must not touch -------------------------
//
// Code first, so that a `$` inside a shell snippet is never read as math; then
// display math before inline, so `$$…$$` is not seen as two empty `$…$`.

function lift(text) {
  const code = [];
  const math = [];
  let out = String(text);

  out = out.replace(/```([^\n`]*)\n?([\s\S]*?)```/g, (_, lang, body) => {
    code.push({ lang: String(lang).trim(), body });
    return `${CODE}${code.length - 1}${END}`;
  });
  out = out.replace(/`([^`\n]+)`/g, (_, body) => {
    code.push({ lang: null, body, inline: true });
    return `${CODE}${code.length - 1}${END}`;
  });

  const push = (body, display) => {
    math.push({ body, display });
    return `${MATH}${math.length - 1}${END}`;
  };
  out = out.replace(/\$\$([\s\S]+?)\$\$/g, (_, body) => push(body, true));
  out = out.replace(/\\\[([\s\S]+?)\\\]/g, (_, body) => push(body, true));
  out = out.replace(/\\\(([\s\S]+?)\\\)/g, (_, body) => push(body, false));
  // Inline `$…$` must not span a blank line, or an unpaired `$` in prose would
  // swallow whole paragraphs. A price ("$5 and $6") stays prose for the same
  // reason: no line break, but also no closing pair on the same line is common.
  out = out.replace(/\$([^$\n]+?)\$/g, (_, body) => push(body, false));

  return { text: out, code, math };
}

// --- step 3: markdown, over already-escaped text ----------------------------

function inline(text) {
  return text
    .replace(/\[([^\]\n]*)\]\(([^)\s]+)\)/g, (whole, label, href) =>
      SAFE_SCHEME.test(href)
        ? `<a href="${href}" target="_blank" rel="noopener noreferrer">${label || href}</a>`
        : whole)
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[^*])\*([^*\n]+)\*/g, "$1<em>$2</em>")
    .replace(/(^|\s)_([^_\n]+)_(?=\s|$|[.,;:!?])/g, "$1<em>$2</em>");
}

function tableRow(line) {
  return line.trim().replace(/^\||\|$/g, "").split("|").map((cell) => cell.trim());
}

function isDivider(line) {
  return /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/.test(line);
}

function blocks(text) {
  const lines = text.split("\n");
  const out = [];
  let index = 0;

  const listItem = (line) => {
    const bullet = /^\s*[-*+]\s+(.*)$/.exec(line);
    if (bullet) return { ordered: false, body: bullet[1] };
    const numbered = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    return numbered ? { ordered: true, body: numbered[1] } : null;
  };

  // Every pattern below is matched against ALREADY-ESCAPED text, so a
  // blockquote marker arrives as `&gt;`. Matching a bare `>` here silently
  // renders every quotation as literal "&gt; ...".
  const QUOTE = /^\s*&gt;\s?/;

  while (index < lines.length) {
    const line = lines[index];

    if (!line.trim()) { index += 1; continue; }

    if (/^\s*(?:---+|\*\*\*+|___+)\s*$/.test(line)) {
      out.push("<hr>"); index += 1; continue;
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      const level = Math.min(heading[1].length + 2, 6);   // # is a chat message, not a document
      out.push(`<h${level}>${inline(heading[2].trim())}</h${level}>`);
      index += 1; continue;
    }

    if (QUOTE.test(line)) {
      const body = [];
      while (index < lines.length && QUOTE.test(lines[index])) {
        body.push(lines[index].replace(QUOTE, "")); index += 1;
      }
      out.push(`<blockquote>${blocks(body.join("\n"))}</blockquote>`);
      continue;
    }

    if (line.includes("|") && index + 1 < lines.length && isDivider(lines[index + 1])) {
      const head = tableRow(line);
      index += 2;
      const body = [];
      while (index < lines.length && lines[index].includes("|") && lines[index].trim()) {
        body.push(tableRow(lines[index])); index += 1;
      }
      const th = head.map((cell) => `<th>${inline(cell)}</th>`).join("");
      const rows = body
        .map((row) => `<tr>${row.map((cell) => `<td>${inline(cell)}</td>`).join("")}</tr>`)
        .join("");
      out.push(`<table class="t"><thead><tr>${th}</tr></thead><tbody>${rows}</tbody></table>`);
      continue;
    }

    const first = listItem(line);
    if (first) {
      const tag = first.ordered ? "ol" : "ul";
      const items = [];
      while (index < lines.length) {
        const item = listItem(lines[index]);
        if (!item || item.ordered !== first.ordered) break;
        const body = [item.body];
        index += 1;
        // A wrapped continuation line belongs to the item it follows.
        while (index < lines.length && lines[index].trim() && !listItem(lines[index])
               && !QUOTE.test(lines[index])
               && !/^(#{1,6}\s|\s*(?:---+|\*\*\*+|___+)\s*$)/.test(lines[index])) {
          body.push(lines[index].trim()); index += 1;
        }
        items.push(`<li>${inline(body.join(" "))}</li>`);
      }
      out.push(`<${tag}>${items.join("")}</${tag}>`);
      continue;
    }

    const paragraph = [];
    while (index < lines.length && lines[index].trim() && !listItem(lines[index])
           && !/^#{1,6}\s/.test(lines[index]) && !QUOTE.test(lines[index])
           && !/^\s*(?:---+|\*\*\*+|___+)\s*$/.test(lines[index])) {
      paragraph.push(lines[index]); index += 1;
    }
    if (paragraph.length) out.push(`<p>${inline(paragraph.join("\n"))}</p>`);
  }
  return out.join("");
}

// --- step 4: put code and math back ----------------------------------------

function restore(html, code, math, katex) {
  let out = html;
  out = out.replace(new RegExp(`${MATH}(\\d+)${END}`, "g"), (_, n) => {
    const item = math[Number(n)];
    if (!item) return "";
    if (!katex) return `<code>${escapeHtml(item.body)}</code>`;
    try {
      // trust:false (the default) is what keeps \href and \htmlClass from
      // reintroducing author-controlled markup. Do not turn it on.
      return katex.renderToString(item.body, {
        displayMode: item.display,
        throwOnError: false,
        errorColor: "var(--danger, #b3261e)",
        output: "html",
      });
    } catch (_) {
      // Unrenderable math is shown as the source the agent wrote, which is
      // strictly better than an empty gap where an equation should be.
      return `<code class="math-error">${escapeHtml(item.body)}</code>`;
    }
  });
  out = out.replace(new RegExp(`${CODE}(\\d+)${END}`, "g"), (_, n) => {
    const item = code[Number(n)];
    if (!item) return "";
    if (item.inline) return `<code>${escapeHtml(item.body)}</code>`;
    const lang = item.lang ? ` data-lang="${escapeHtml(item.lang)}"` : "";
    return `<pre${lang}><code>${escapeHtml(item.body.replace(/\n$/, ""))}</code></pre>`;
  });
  return out;
}

/**
 * Render one message body. `katex` is injected so this file has no import of
 * its own and can be exercised without a DOM.
 */
export function renderMarkdownWithMath(text, katex) {
  const lifted = lift(text ?? "");
  return restore(blocks(escapeHtml(lifted.text)), lifted.code, lifted.math, katex);
}

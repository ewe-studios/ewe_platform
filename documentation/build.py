#!/usr/bin/env python3
"""Convert markdown files to HTML. Zero dependencies -- stdlib only.

Usage:
    python3 build.py [PROJECT_DIR]

Where PROJECT_DIR is like 'documentation/pi' or 'documentation/hermes'.
Defaults to the directory this script lives in.
"""

import re
import os
import sys
from html import escape
from pathlib import Path


# ── HTML template ────────────────────────────────────────────────────────────

HTML_TEMPLATE = """\
<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <link rel="stylesheet" href="styles.css">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.css">
  <script>
    (function() {{
      var s = localStorage.getItem('theme');
      var p = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
      document.documentElement.setAttribute('data-theme', s || p);
    }})();
  </script>
  <script type="module">
    (async function() {{
      var blocks = document.querySelectorAll('.mermaid');
      if (blocks.length === 0) return;
      var {{ default: m }} = await import('https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs');
      function cfg() {{
        var d = document.documentElement;
        var dark = d.getAttribute('data-theme') === 'dark';
        var s = getComputedStyle(d);
        var bg = s.getPropertyValue(dark ? '--bg-strong' : '--bg-strong').trim();
        var fg = s.getPropertyValue('--fg').trim();
        var line = s.getPropertyValue('--line-strong').trim();
        var muted = s.getPropertyValue('--bg-muted').trim();
        return {{ startOnLoad: false, securityLevel: 'strict', htmlLabels: false, theme: 'base', flowchart: {{ curve: 'linear' }}, themeVariables: {{
          primaryColor: bg, primaryTextColor: fg, primaryBorderColor: line,
          lineColor: line, secondaryColor: muted, tertiaryColor: s.getPropertyValue('--bg').trim(),
          fontFamily: "'JetBrains Mono', monospace" }} }};
      }}
      blocks.forEach(function(b) {{ b.setAttribute('data-src', b.textContent); }});
      async function render() {{
        m.initialize(cfg());
        blocks.forEach(function(b) {{ b.removeAttribute('data-processed'); b.innerHTML = b.getAttribute('data-src'); }});
        await m.run({{ nodes: blocks }});
      }}
      await render();
      new MutationObserver(render).observe(document.documentElement, {{ attributes: true, attributeFilter: ['data-theme'] }});
      // ── Diagram zoom/pan modal ──────────────────────────────
      var overlay = null, container = null, svg = null;
      var scale = 1, panX = 0, panY = 0, dragging = false, startX, startY;
      function openModal(src) {{
        if (overlay) return;
        overlay = document.createElement('div');
        overlay.className = 'diagram-modal-overlay';
        overlay.setAttribute('role', 'dialog');
        container = document.createElement('div');
        container.className = 'diagram-modal-container';
        svg = src.cloneNode(true);
        container.appendChild(svg);
        var toolbar = document.createElement('div');
        toolbar.className = 'diagram-modal-toolbar';
        var btnIn = mkBtn('+', function(e) {{ e.stopPropagation(); zoom(1.3); }});
        var btnOut = mkBtn('−', function(e) {{ e.stopPropagation(); zoom(0.7); }});
        var btnReset = mkBtn('fit', function(e) {{ e.stopPropagation(); resetZoom(); }});
        var btnClose = mkBtn('✕', function(e) {{ e.stopPropagation(); closeModal(); }});
        toolbar.appendChild(btnIn); toolbar.appendChild(btnOut);
        toolbar.appendChild(btnReset); toolbar.appendChild(btnClose);
        container.appendChild(toolbar);
        var hint = document.createElement('div');
        hint.className = 'diagram-modal-hint';
        hint.textContent = 'Scroll to zoom · Drag to pan · Esc to close';
        document.body.appendChild(overlay);
        overlay.appendChild(container);
        document.body.appendChild(hint);
        scale = 1; panX = 0; panY = 0;
        container.addEventListener('mousedown', onDragStart);
        container.addEventListener('wheel', onWheel, {{ passive: false }});
        overlay.addEventListener('mousedown', onOverlayClick);
        document.addEventListener('keydown', onKey);
        setTimeout(function() {{
          var fit = Math.min(window.innerWidth * 0.9 / svg.clientWidth, window.innerHeight * 0.85 / svg.clientHeight, 1);
          scale = fit; applyTransform();
        }}, 50);
      }}
      function mkBtn(label, handler) {{
        var b = document.createElement('button');
        b.className = 'diagram-modal-btn'; b.textContent = label;
        b.addEventListener('click', handler); return b;
      }}
      function zoom(f) {{ scale *= f; applyTransform(); }}
      function resetZoom() {{
        var fit = Math.min(window.innerWidth * 0.9 / svg.clientWidth, window.innerHeight * 0.85 / svg.clientHeight, 1);
        scale = fit; panX = 0; panY = 0; applyTransform();
      }}
      function applyTransform() {{ svg.style.transform = 'translate(' + panX + 'px,' + panY + 'px) scale(' + scale + ')'; }}
      function onWheel(e) {{ e.preventDefault(); zoom(e.deltaY < 0 ? 1.15 : 0.87); }}
      function onDragStart(e) {{
        if (e.target.tagName === 'BUTTON') return;
        dragging = true; startX = e.clientX - panX; startY = e.clientY - panY;
        container.classList.add('dragging');
        document.addEventListener('mousemove', onDragMove);
        document.addEventListener('mouseup', onDragEnd);
      }}
      function onDragMove(e) {{ panX = e.clientX - startX; panY = e.clientY - startY; applyTransform(); }}
      function onDragEnd() {{ dragging = false; container.classList.remove('dragging');
        document.removeEventListener('mousemove', onDragMove);
        document.removeEventListener('mouseup', onDragEnd);
      }}
      function onOverlayClick(e) {{ if (e.target === overlay) closeModal(); }}
      function onKey(e) {{ if (e.key === 'Escape') closeModal(); }}
      function closeModal() {{
        if (!overlay) return;
        overlay.remove(); overlay = null; container = null; svg = null;
        document.removeEventListener('keydown', onKey);
        document.removeEventListener('mousemove', onDragMove);
        document.removeEventListener('mouseup', onDragEnd);
      }}
      setTimeout(function() {{
        document.querySelectorAll('.mermaid').forEach(function(el) {{
          el.addEventListener('click', function() {{
            var rendered = el.querySelector('svg');
            if (rendered) openModal(rendered);
          }});
        }});
      }}, 500);
    }})();
  </script>
</head>
<body>
  <nav class="nav">
    <span class="nav-brand">~/{project}/docs</span>
    <div class="nav-actions">
      {breadcrumbs}
      <a href="index.html" class="nav-btn" title="Back to index">index</a>
      {prev_btn}
      {next_btn}
      <button class="theme-toggle" onclick="toggleTheme()" title="Toggle dark/light theme">theme</button>
    </div>
  </nav>
  <article class="prose">
{content}
  </article>
  <script>function toggleTheme(){{var c=document.documentElement.getAttribute('data-theme'),n=c==='dark'?'light':'dark';document.documentElement.setAttribute('data-theme',n);localStorage.setItem('theme',n);}}</script>
</body>
</html>
"""

INDEX_TEMPLATE = """\
<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{project} -- Documentation</title>
  <link rel="stylesheet" href="styles.css">
  <script>
    (function() {{
      var s = localStorage.getItem('theme');
      var p = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
      document.documentElement.setAttribute('data-theme', s || p);
    }})();
  </script>
</head>
<body>
  <nav class="nav">
    <span class="nav-brand">~/{project}/docs</span>
    <div class="nav-actions">
      <button class="theme-toggle" onclick="toggleTheme()" title="Toggle dark/light theme">theme</button>
    </div>
  </nav>

  <h1>{project} Documentation</h1>
  <p>{description}</p>
  <ul class="doc-index">
    {links}
  </ul>

  <hr>
  <p style="color: var(--fg-soft); font-size: 0.85rem;">
    Generated from markdown. Mermaid diagrams require JavaScript. Dark mode supported.
  </p>
  <script>function toggleTheme(){{var c=document.documentElement.getAttribute('data-theme'),n=c==='dark'?'light':'dark';document.documentElement.setAttribute('data-theme',n);localStorage.setItem('theme',n);}}</script>
</body>
</html>
"""


# ── Markdown → HTML converter ───────────────────────────────────────────────

class Md2Html:
    """Minimal markdown-to-HTML converter. Handles the subset our docs use."""

    def convert(self, text: str) -> str:
        self._mermaid_counter = 0

        # Extract fenced code blocks first to protect them from other processing
        parts = []
        last_end = 0
        for m in re.finditer(r'^```(mermaid)?(\w+)?\s*\n(.*?)^```\s*$', text, re.DOTALL | re.MULTILINE):
            if m.start() > last_end:
                parts.append(('normal', text[last_end:m.start()]))
            lang = m.group(1) if m.group(1) else m.group(2) or ''
            code = m.group(3)
            parts.append(('code', lang, code))
            last_end = m.end()
        if last_end < len(text):
            parts.append(('normal', text[last_end:]))

        result_parts = []
        for part in parts:
            if part[0] == 'code':
                _, lang, code = part
                escaped = escape(code)
                if lang == 'mermaid':
                    self._mermaid_counter += 1
                    result_parts.append(f'<div class="mermaid">\n{escape(code)}\n</div>')
                else:
                    result_parts.append(
                        f'<pre><code class="language-{lang}">{escaped}</code></pre>'
                    )
            else:
                result_parts.append(self._convert_block(part[1]))

        return '\n'.join(result_parts)

    def _convert_block(self, text: str) -> str:
        lines = text.split('\n')
        result = []
        i = 0
        in_table = False
        table_rows = []
        in_list = False
        list_type = 'ul'
        list_items = []
        in_blockquote = False
        bq_lines = []

        def flush_table():
            if table_rows:
                rows_html = []
                for idx, row in enumerate(table_rows):
                    tag = 'th' if idx == 0 else 'td'
                    cells = ''.join(f'<{tag}>{c.strip()}</{tag}>' for c in row.split('|') if c.strip())
                    rows_html.append(f'<tr>{cells}</tr>')
                result.append('<table>' + ''.join(rows_html) + '</table>')
                table_rows.clear()

        def flush_list():
            if list_items:
                tag = 'ul' if list_type == 'ul' else 'ol'
                result.append(f'<{tag}>{"".join(f"<li>{self._inline(item)}</li>" for item in list_items)}</{tag}>')
                list_items.clear()

        def flush_bq():
            if bq_lines:
                inner = self._inline('\n'.join(bq_lines))
                result.append(f'<blockquote>{inner}</blockquote>')
                bq_lines.clear()

        while i < len(lines):
            line = lines[i]

            # Empty line -- flush any open block
            if not line.strip():
                flush_table()
                flush_list()
                flush_bq()
                in_table = False
                in_list = False
                in_blockquote = False
                i += 1
                continue

            # Horizontal rule
            if re.match(r'^-{3,}$', line):
                flush_table(); flush_list(); flush_bq()
                result.append('<hr>')
                i += 1
                continue

            # Headings
            h = re.match(r'^(#{1,4})\s+(.+)', line)
            if h:
                flush_table(); flush_list(); flush_bq()
                level = len(h.group(1))
                result.append(f'<h{level}>{self._inline(h.group(2))}</h{level}>')
                i += 1
                continue

            # Table row
            if '|' in line and i + 1 < len(lines) and re.match(r'^\|[\s\-:|]+\|', lines[i + 1]):
                flush_list(); flush_bq()
                in_table = True
                table_rows.append(line)
                i += 2  # skip separator line
                # Collect remaining rows
                while i < len(lines) and '|' in lines[i] and not lines[i].strip().startswith('###'):
                    table_rows.append(lines[i])
                    i += 1
                flush_table()
                in_table = False
                continue

            if '|' in line and in_table:
                table_rows.append(line)
                i += 1
                continue

            # Blockquote
            if line.startswith('> '):
                flush_table(); flush_list()
                in_blockquote = True
                bq_lines.append(line[2:])
                i += 1
                continue

            # List item
            list_match = re.match(r'^(\s*)([\*\-]|(\d+)\.)\s+(.*)', line)
            if list_match:
                flush_table(); flush_bq()
                indent = list_match.group(1)
                num = list_match.group(3)
                item = list_match.group(4)
                if num:
                    if list_type != 'ol':
                        flush_list()
                        list_type = 'ol'
                    in_list = True
                else:
                    if list_type != 'ul':
                        flush_list()
                        list_type = 'ul'
                    in_list = True
                list_items.append(item)
                i += 1
                continue

            # Normal paragraph text
            flush_table(); flush_bq()
            flush_list()
            # Collect consecutive non-special lines into one paragraph
            para_lines = [line]
            i += 1
            while i < len(lines):
                nl = lines[i]
                if (not nl.strip() or re.match(r'^#{1,4}\s', nl) or
                    nl.startswith('> ') or re.match(r'^(\s*)([\*\-]|\d+\.)\s', nl) or
                    re.match(r'^-{3,}$', nl)):
                    break
                para_lines.append(nl)
                i += 1
            result.append(f'<p>{self._inline(" ".join(para_lines))}</p>')

        flush_table(); flush_list(); flush_bq()
        return '\n'.join(result)

    def _inline(self, text: str) -> str:
        # Inline code
        text = re.sub(r'`([^`]+)`', r'<code>\1</code>', text)
        # Bold
        text = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', text)
        # Italic
        text = re.sub(r'\*(.+?)\*', r'<em>\1</em>', text)
        # Links
        text = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', r'<a href="\2">\1</a>', text)
        # Relative md links in docs: ../markdown/XX-slug.md → XX-slug.html
        text = re.sub(r'\[([^\]]+)\]\((\.+/)*markdown/([^)]+\.md)\)',
                      lambda m: f'<a href="{os.path.splitext(m.group(3))[0]}.html">{m.group(1)}</a>',
                      text)
        return text


# ── Build logic ──────────────────────────────────────────────────────────────

def build(project_dir: str):
    project_dir = Path(project_dir).resolve()
    md_dir = project_dir / 'markdown'
    html_dir = project_dir / 'html'
    if not md_dir.exists():
        print(f"Error: {md_dir} does not exist")
        sys.exit(1)

    html_dir.mkdir(parents=True, exist_ok=True)
    converter = Md2Html()
    project_name = project_dir.name
    print(f"Building {project_name} docs: {md_dir} → {html_dir}")

    # Discover markdown files
    md_files = sorted(md_dir.glob('*.md'))
    if not md_files:
        print("No markdown files found.")
        return

    # Build each markdown file to HTML
    file_map = {}  # slug → (filename, title)
    for md_file in md_files:
        with open(md_file) as f:
            content = f.read()

        # Extract title from frontmatter or first heading
        fm = re.match(r'^---\s*\n(.*?)\n---\s*\n', content, re.DOTALL)
        title = ''
        if fm:
            title_m = re.search(r'^title:\s*"?([^"\n]+)"?', fm.group(1), re.MULTILINE)
            if title_m:
                title = title_m.group(1).strip()
        if not title:
            h = re.search(r'^#\s+(.+)', content, re.MULTILINE)
            if h:
                title = h.group(1).strip()
        if not title:
            title = md_file.stem

        slug = md_file.stem  # e.g., "00-overview"
        file_map[slug] = (md_file.name, title)

    # Pre-compute ordered slug list for prev/next navigation
    all_slugs = list(file_map.keys())

    for md_file in md_files:
        with open(md_file) as f:
            content = f.read()

        # Extract title (already computed above, but needed for content)
        fm = re.match(r'^---\s*\n(.*?)\n---\s*\n', content, re.DOTALL)
        slug = md_file.stem
        title = file_map[slug][1]

        # Convert markdown to HTML
        body = content
        if fm:
            body = content[fm.end():]  # Strip frontmatter

        html_body = converter.convert(body)

        # Navigation: breadcrumbs, index btn, prev btn, next btn
        idx = all_slugs.index(slug)

        # Breadcrumbs: last 3 pages before current one
        breadcrumbs = ''
        for p in all_slugs[max(0, idx-3):idx]:
            breadcrumbs += f'<a href="{p}.html" class="nav-breadcrumb">{file_map[p][1]}</a>'

        # Previous button
        if idx > 0:
            prev_slug = all_slugs[idx - 1]
            prev_btn = f'<a href="{prev_slug}.html" class="nav-btn" title="Previous: {file_map[prev_slug][1]}">← prev</a>'
        else:
            prev_btn = ''

        # Next button
        if idx < len(all_slugs) - 1:
            next_slug = all_slugs[idx + 1]
            next_btn = f'<a href="{next_slug}.html" class="nav-btn nav-btn-next" title="Next: {file_map[next_slug][1]}">next →</a>'
        else:
            next_btn = ''

        html_content = HTML_TEMPLATE.format(
            title=f"{title} -- {project_name}",
            project=project_name,
            content=html_body,
            breadcrumbs=breadcrumbs,
            prev_btn=prev_btn,
            next_btn=next_btn,
        )

        out_file = html_dir / f"{slug}.html"
        with open(out_file, 'w') as f:
            f.write(html_content)
        print(f"  ✓ {slug}.html  ({title})")

    # Build index page
    links_html = ''
    for slug, (_, title) in file_map.items():
        links_html += f'<li><a href="{slug}.html">{title}</a></li>\n'

    descriptions = {
        'pi': "Modular AI agent framework. 7 TypeScript packages for LLM APIs, agent runtimes, and applications.",
        'hermes': "Self-improving AI agent. Python framework with 40+ tools, 10+ messaging platforms, and pluggable memory.",
        'autoresearch': "Autonomous AI research system. AI agent experiments with LLM training code overnight, ~100 experiments/night.",
        'open-pencil': "Open-source design editor. Opens .fig/.pen files, 100+ AI tools, MCP server, WebRTC collaboration, headless CLI + Vue SDK.",
        'paperclip': "Open-source AI company orchestration. Org charts, budgets, governance, and coordination for multi-agent teams.",
        'graphify': "Knowledge graph extraction tool. Turns mixed-media corpora into queryable graphs via tree-sitter AST, Whisper transcription, and Claude semantic extraction. 71.5x token reduction, 25 languages, 14 platform integrations.",
        'mastra': "TypeScript-first AI agent framework. Unified model router, workflow-based agentic loop, built-in memory, processor pipeline, and multi-model fallbacks.",
        'resonate': "Distributed computing framework. Durable execution, task coordination, and event-driven workflows for reliable background processing.",
        'rust-authz': "Four Rust crates: Zanzibar-style FGA authorization engine (authz-core), PostgreSQL extension (pgauthz), auto-generated REST API for databases (dbrest), and telemetry ingestion platform (zradar).",
        'aipack': "Jeremy Chone's Rust crate collection: genai (19 AI providers), rpc-router (JSON-RPC 2.0), sqlb (SQL builder), modql (query language), agentic (MCP protocol), udiffx (diff parser), and 7 utility crates.",
        'voice-agent-server': "Voice AI assistant server. Express.js REST API managing Vapi voice assistants and phone numbers with 11Labs synthesis.",
    }
    desc = descriptions.get(project_name, f"Documentation for {project_name}.")

    index_html = INDEX_TEMPLATE.format(
        project=project_name,
        description=desc,
        links=links_html,
    )

    with open(html_dir / 'index.html', 'w') as f:
        f.write(index_html)
    print(f"  ✓ index.html")

    # Copy shared CSS if not already present
    css_file = html_dir / 'styles.css'
    if not css_file.exists():
        css = (Path(__file__).resolve().parent / 'styles.css').read_text()
        css_file.write_text(css)
        print(f"  ✓ styles.css")

    print(f"\nDone. Open {html_dir / 'index.html'} in a browser.")


if __name__ == '__main__':
    if len(sys.argv) > 1:
        build(sys.argv[1])
    else:
        # Default: build pi, hermes, open-pencil, and autoresearch from this directory
        base = Path(__file__).resolve().parent
        for proj in ['pi', 'hermes', 'open-pencil', 'autoresearch', 'paperclip', 'voice-agent-server', 'graphify', 'mastra', 'resonate', 'rust-authz', 'aipack']:
            p = base / proj
            if p.exists():
                build(str(p))
                print()

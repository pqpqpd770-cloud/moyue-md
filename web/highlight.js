/* Tiny dependency-free highlighter shared by the previewer and the exported page. */
(function () {
  'use strict';

  const KEYWORDS = {
    rust: ['as', 'async', 'await', 'box', 'break', 'const', 'continue', 'crate', 'dyn', 'else', 'enum', 'extern',
      'false', 'fn', 'for', 'if', 'impl', 'in', 'let', 'loop', 'match', 'mod', 'move', 'mut', 'pub', 'ref',
      'return', 'self', 'Self', 'static', 'struct', 'super', 'trait', 'true', 'type', 'unsafe', 'use', 'where', 'while'],
    python: ['and', 'as', 'assert', 'async', 'await', 'break', 'class', 'continue', 'def', 'del', 'elif', 'else',
      'except', 'False', 'finally', 'for', 'from', 'global', 'if', 'import', 'in', 'is', 'lambda', 'None',
      'nonlocal', 'not', 'or', 'pass', 'raise', 'return', 'True', 'try', 'while', 'with', 'yield'],
    javascript: ['async', 'await', 'break', 'case', 'catch', 'class', 'const', 'continue', 'default', 'delete', 'do',
      'else', 'export', 'extends', 'false', 'finally', 'for', 'function', 'if', 'import', 'in', 'instanceof',
      'let', 'new', 'null', 'return', 'super', 'switch', 'this', 'throw', 'true', 'try', 'typeof', 'undefined',
      'var', 'void', 'while', 'yield'],
    c: ['auto', 'break', 'case', 'char', 'const', 'continue', 'default', 'do', 'double', 'else', 'enum', 'extern',
      'float', 'for', 'goto', 'if', 'int', 'long', 'register', 'return', 'short', 'signed', 'sizeof', 'static',
      'struct', 'switch', 'typedef', 'union', 'unsigned', 'void', 'volatile', 'while', 'class', 'namespace',
      'template', 'typename', 'public', 'private', 'protected', 'new', 'delete', 'this', 'using', 'nullptr',
      'bool', 'override', 'constexpr', 'virtual'],
    go: ['break', 'case', 'chan', 'const', 'continue', 'default', 'defer', 'else', 'fallthrough', 'for', 'func',
      'go', 'goto', 'if', 'import', 'interface', 'map', 'package', 'range', 'return', 'select', 'struct',
      'switch', 'type', 'var', 'nil', 'true', 'false'],
    shell: ['if', 'then', 'else', 'elif', 'fi', 'for', 'while', 'do', 'done', 'case', 'esac', 'function',
      'in', 'return', 'export', 'local', 'echo', 'cd', 'set', 'source'],
    sql: ['select', 'from', 'where', 'insert', 'update', 'delete', 'create', 'table', 'index', 'join', 'left',
      'right', 'inner', 'outer', 'group', 'order', 'by', 'having', 'limit', 'values', 'into', 'and', 'or',
      'not', 'null', 'primary', 'key', 'foreign', 'references', 'default'],
    yaml: ['true', 'false', 'null', 'yes', 'no'],
  };

  const HASH_COMMENT = /^(py|python|sh|bash|zsh|shell|yaml|yml|toml|rb|ruby|pl|perl|makefile|dockerfile|conf|ini)$/;
  const BUILTINS = new Set(['json', 'true', 'false', 'null', 'nil', 'self', 'print']);
  const KNOWN = new Set(Object.keys(KEYWORDS));
  const PLAIN = new Set(['', 'text', 'txt', 'plain', 'plaintext', 'none', 'log', 'output']);

  function alias(lang) {
    if (lang === 'js' || lang === 'jsx' || lang === 'ts' || lang === 'tsx' || lang === 'typescript') return 'javascript';
    if (lang === 'rs') return 'rust';
    if (lang === 'cpp' || lang === 'c++' || lang === 'cc' || lang === 'cxx' || lang === 'h' || lang === 'hpp') return 'c';
    if (lang === 'py') return 'python';
    if (lang === 'sh' || lang === 'bash' || lang === 'zsh') return 'shell';
    if (lang === 'yml') return 'yaml';
    return lang;
  }

  function escapeHtml(text) {
    return text.replace(/[&<>]/g, (c) => (c === '&' ? '&amp;' : c === '<' ? '&lt;' : '&gt;'));
  }

  function highlight(source, lang) {
    const key = alias(lang);
    const words = new Set(KEYWORDS[key] || []);
    const hashComments = HASH_COMMENT.test(lang);
    const pattern = [
      hashComments ? '(#[^\\n]*)' : '(\\/\\/[^\\n]*|\\/\\*[\\s\\S]*?\\*\\/|--[^\\n]*)',
      '(#\\s*(?:include|define|pragma|ifdef|ifndef|endif|else|elif|error|undef)[^\\n]*)',
      '("(?:\\\\.|[^"\\\\\\n])*"|\'(?:\\\\.|[^\'\\\\\\n])*\'|`(?:\\\\.|[^`\\\\])*`)',
      '(\\b\\d[\\d_]*(?:\\.\\d+)?(?:[eE][+-]?\\d+)?\\b|\\b0[xX][0-9a-fA-F_]+\\b)',
      '([A-Za-z_$\\u4e00-\\u9fa5][\\w$]*)',
    ].join('|');
    const re = new RegExp(pattern, 'gm');

    let out = '';
    let last = 0;
    let match;
    while ((match = re.exec(source)) !== null) {
      out += escapeHtml(source.slice(last, match.index));
      const full = match[0];
      const comment = match[1];
      const preproc = match[2];
      const str = match[3];
      const num = match[4];
      const word = match[5];
      if (comment) out += '<span class="tok-com">' + escapeHtml(full) + '</span>';
      else if (preproc) out += '<span class="tok-pp">' + escapeHtml(full) + '</span>';
      else if (str) out += '<span class="tok-str">' + escapeHtml(full) + '</span>';
      else if (num) out += '<span class="tok-num">' + escapeHtml(full) + '</span>';
      else if (word) {
        if (words.has(word) || words.has(word.toLowerCase())) out += '<span class="tok-kw">' + escapeHtml(word) + '</span>';
        else if (BUILTINS.has(word)) out += '<span class="tok-typ">' + escapeHtml(word) + '</span>';
        else if (/^[A-Z][A-Za-z0-9_]*$/.test(word)) out += '<span class="tok-typ">' + escapeHtml(word) + '</span>';
        else if (source[re.lastIndex] === '(') out += '<span class="tok-fn">' + escapeHtml(word) + '</span>';
        else out += escapeHtml(word);
      } else {
        out += escapeHtml(full);
      }
      last = re.lastIndex;
      if (full.length === 0) re.lastIndex += 1;
    }
    out += escapeHtml(source.slice(last));
    return out;
  }

  /* Highlight every fenced code block inside `root` and tag their languages. */
  /*
   * Sub/superscript that sits inside a word.
   *
   * The markdown parser follows CommonMark's intraword rule, so `H~2~O` and
   * `x^2^` are left alone while `a ~sub~ b` is parsed. Chemistry and maths need
   * the intraword form, so anything the parser skipped is turned into
   * <sub>/<sup> here. Double tildes (~~strike~~) are left untouched, and code
   * blocks are never touched because <code>/<pre> is filtered out below.
   */
  const TYPESET = /(?<!~)~([^\s~][^~]{0,60}?)~(?!~)|\^([^\s^][^^]{0,60}?)\^/g;

  function typeset(root) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const text = node.nodeValue;
        if (!text || (text.indexOf('~') < 0 && text.indexOf('^') < 0)) return NodeFilter.FILTER_REJECT;
        const parent = node.parentElement;
        if (!parent || parent.closest('code, pre, script, style')) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      },
    });

    const pending = [];
    let node;
    while ((node = walker.nextNode())) pending.push(node);

    pending.forEach((textNode) => {
      const text = textNode.nodeValue;
      TYPESET.lastIndex = 0;
      const pieces = [];
      let last = 0;
      let match;
      while ((match = TYPESET.exec(text)) !== null) {
        if (match.index > last) pieces.push(document.createTextNode(text.slice(last, match.index)));
        const tag = document.createElement(match[1] !== undefined ? 'sub' : 'sup');
        tag.textContent = match[1] !== undefined ? match[1] : match[2];
        pieces.push(tag);
        last = TYPESET.lastIndex;
      }
      if (!pieces.length) return;
      if (last < text.length) pieces.push(document.createTextNode(text.slice(last)));
      const fragment = document.createDocumentFragment();
      pieces.forEach((piece) => fragment.appendChild(piece));
      textNode.parentNode.replaceChild(fragment, textNode);
    });
  }

  /* Copy helper: the app falls back to the Rust clipboard, the exported page
     falls back to the browser. */
  function copyText(text) {
    // Inside the app the native clipboard is the reliable path; browser
    // clipboard rules do not apply to a page served from memory.
    if (window.ipc) {
      window.ipc.postMessage(JSON.stringify({ cmd: 'copy', value: text }));
      return;
    }

    const scratchCopy = () => {
      const scratch = document.createElement('textarea');
      scratch.value = text;
      scratch.setAttribute('readonly', '');
      scratch.style.position = 'fixed';
      scratch.style.top = '-1000px';
      document.body.appendChild(scratch);
      scratch.select();
      let ok = false;
      try {
        ok = document.execCommand('copy');
      } catch (err) {
        ok = false;
      }
      scratch.remove();
      return ok;
    };

    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(text).catch(() => {
        scratchCopy();
      });
      return;
    }
    scratchCopy();
  }

  function decorate(root) {
    root.querySelectorAll('pre').forEach((pre) => {
      const code = pre.querySelector('code');
      if (!code) return;
      const found = /language-([\w+#-]+)/.exec(code.className || '');
      const lang = found ? found[1].toLowerCase() : '';
      const source = code.textContent;
      // Only colourise languages we actually know. A directory tree or a log
      // pasted into a bare fence should stay plain monospace.
      if (KNOWN.has(alias(lang)) && !PLAIN.has(lang)) {
        code.innerHTML = highlight(source, lang);
      } else {
        code.textContent = source;
      }

      if (pre.parentElement && pre.parentElement.classList.contains('codebox')) return;

      const box = document.createElement('div');
      box.className = 'codebox';
      const bar = document.createElement('div');
      bar.className = 'codebar';
      const label = document.createElement('span');
      label.className = 'lang';
      // 纯文本、目录树这类没标语言的块不挂标签，免得每块都写着 text
      label.textContent = PLAIN.has(lang) ? '' : lang;
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'copy';
      button.textContent = '复制';
      button.addEventListener('click', () => {
        copyText(source);
        button.textContent = '已复制';
        button.classList.add('done');
        window.setTimeout(() => {
          button.textContent = '复制';
          button.classList.remove('done');
        }, 1400);
      });
      bar.append(label, button);

      pre.parentNode.insertBefore(box, pre);
      box.append(bar, pre);
    });
    root.querySelectorAll('li > input[type="checkbox"]').forEach((box) => {
      box.parentElement.classList.add('task-list-item');
    });
    relaxPaths(root);
    typeset(root);
    tightenWideCells(root);
  }

  /*
   * A cell holding one very long latin token (CompressedInMemory, a long path,
   * a URL) reports a huge max-content width, and the table algorithm then hands
   * it most of the space while a prose column is squeezed to a sliver. Marking
   * those cells as breakable lets the columns share the width sensibly.
   */
  function tightenWideCells(root) {
    root.querySelectorAll('th, td').forEach((cell) => {
      const text = cell.textContent || '';
      let run = 0;
      let longest = 0;
      for (const ch of text) {
        if (/[A-Za-z0-9_./<>*+=:#@-]/.test(ch)) {
          run += 1;
          if (run > longest) longest = run;
        } else {
          run = 0;
        }
      }
      if (longest >= 15) cell.classList.add('tight');
    });
  }

  /*
   * Long paths such as Assets/StreamingAssets/languageJsonFile/Subtitles.json
   * have no break opportunity, so a table cell holding one used to force the
   * whole table wider than the page. <wbr> after each slash gives the browser a
   * cheap break point; it carries no character, so copying still yields the
   * original path.
   */
  function relaxPaths(root) {
    root.querySelectorAll('code').forEach((code) => {
      if (code.closest('pre')) return;
      const text = code.textContent || '';
      if (text.length < 14 || !text.includes('/')) return;
      const fragment = document.createDocumentFragment();
      text.split(/(?<=\/)/).forEach((piece, index) => {
        if (index > 0) fragment.appendChild(document.createElement('wbr'));
        fragment.appendChild(document.createTextNode(piece));
      });
      code.replaceChildren(fragment);
    });
  }

  window.moyueHighlight = highlight;
  window.moyueDecorate = decorate;
  window.moyueCopy = copyText;
})();

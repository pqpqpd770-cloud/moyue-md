# 印本风组件配方

都来自「墨阅」的线上实现，在 Windows / WebView2 上跑过真实文档。约定：圆角默认 3px，`1px solid var(--rule)` 是界行，卡片式结构一律直角 + 无阴影，全窗口同一个 `--paper` 底色。

## 1. 一张纸（窗口骨架）

没有"桌面"，也没有卡片：标题栏、目录、正文同底色，靠留白和细线分区。

```
┌─[印] 文件名 ⌄ › 当前小节 ────────────── 目录 查找 译 Aa ⋯ │ — ▢ ✕ ┐  40px
├═════════════ 花青进度（就是标题栏下边线）══════════════════════════┤  2px
│ 目录                │                                               │
│  文档题名           │        # 标题                                 │
│ ┃当前章节           │        正文……                                 │
└─────────────────────┴───────────────────────────────────────────────┘
```

```css
html, body { height: 100%; margin: 0; background: var(--paper); color: var(--ink); font-family: var(--ui); }
#app { display: flex; flex-direction: column; height: 100%; }
#workspace { flex: 1 1 auto; display: flex; min-height: 0; }
#stage { flex: 1 1 auto; position: relative; min-width: 0; }
#view { height: 100%; overflow: auto; }
#page { max-width: 36em; margin: 0 auto; padding: 32px 40px 120px; }
```

**不要**再引入"外壳色"（`--desk` 那一类）：一旦窗口里出现第二种底色，界面立刻变成"被切成几块"。

## 2. 书眉（最有辨识度的一个装置）

标题栏左边是"书名 + 正在读的那一节"，像书页顶上印的章名。切小节时淡出再淡入。点书名弹出一个面板（路径、编码、字数、行数、阅读时间、最近打开、打开其他文件）——**原来的状态栏就可以删掉了**，正文多出一行高度。

```html
<div id="crumb">
  <button class="crumb-file" id="t-file"><span id="chip-name"></span><svg class="caret">…</svg></button>
  <svg class="crumb-sep">›</svg>
  <span id="crumb-sec"></span>
</div>
```

```js
/* 当前小节：按滚动位置在标题数组里二分查找，往回滚也要能正确更新 */
function currentSection(scrollTop) {
  let lo = 0, hi = headings.length - 1, found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (headings[mid].offsetTop <= scrollTop + 60) { found = mid; lo = mid + 1; } else { hi = mid - 1; }
  }
  return found >= 0 ? headings[found] : null;
}
```

切小节时用 160ms 的透明度过渡换文字，不要做滑动。

## 3. 目录：不编号

中文文档里"H1 当题目、其余是节"占绝大多数，编号一定会出现一个孤零零的"一"。所以**只靠缩进表示层级**：文档第一个 H1 当题目（加粗、不缩进），其余按相对层级缩进，当前节用花青竖线 + 花青文字。

```css
.toc-item {
  display: flex; gap: 8px; padding: 5px 8px;
  border-left: 2px solid transparent; border-radius: 0 3px 3px 0;
  color: var(--ink-2); font-size: 12.5px; cursor: pointer;
  transition: background-color .16s var(--ease), color .16s var(--ease);
}
.toc-item:hover { background: var(--paper-2); color: var(--ink); }
.toc-item.active { color: var(--indigo); border-left-color: var(--indigo); background: var(--indigo-soft); }
.toc-item.title { font-weight: 600; color: var(--ink); }   /* 当题目的那个 H1 */
```

## 4. 三线表 + 长内容断行

只有横线：表头上下 1.5px，行间 1px，末行 1.5px。没有竖线、没有斑马纹。

```css
table { width: 100%; margin: 1.4em 0; border-collapse: collapse; font-size: .94em; }
thead th { padding: 7px 10px; text-align: left; font-weight: 700;
           border-top: 1.5px solid var(--rule-2); border-bottom: 1.5px solid var(--rule-2); }
tbody td { padding: 7px 10px; border-bottom: 1px solid var(--rule); }
tbody tr:last-child td { border-bottom: 1.5px solid var(--rule-2); }
```

**让宽表收进版心**（不做这步，长路径会把表格撑破、整页多出横向滚动条）：

```css
th, td { overflow-wrap: break-word; }      /* 必要时才断词 */
td.tight { overflow-wrap: anywhere; }      /* 含超长英文串的单元格 */
```

`td.tight` 由脚本判断：扫描单元格文本里最长的拉丁串（`[A-Za-z0-9_./<>*+=:#@-]` 连续段），≥15 个字符就加类。作用是让这种单元格的 max-content 塌下来，不再把列宽全抢走——否则一个 `CompressedInMemory` 就能把邻列挤成 110px 宽的一条缝。**表头不要加这个类**，否则 `compressionFormat` 会被折成三行。

行内代码里的路径还要在斜杠处留断点：

```js
// <wbr> 不产生字符，复制出来仍是原路径
if (text.length > 14 && text.includes('/')) {
  const frag = document.createDocumentFragment();
  text.split(/(?<=\/)/).forEach((piece, i) => {
    if (i) frag.appendChild(document.createElement('wbr'));
    frag.appendChild(document.createTextNode(piece));
  });
  code.replaceChildren(frag);
}
```

## 5. 代码块：没有标题栏

不要给代码块加一条 header（那是编辑器味）。语言名淡淡地挂在右上角，鼠标悬停时**同一位置换成"复制"**——省一条线，也省一次眼睛移动。没写语言的块不显示标签，也不上色（目录树、日志粘进来时通用高亮会把内容染得乱七八糟）。

```html
<div class="code">
  <pre><code class="language-rust">…</code></pre>
  <span class="lang">rust</span>
  <button class="copy">复制</button>
</div>
```

```css
.code { position: relative; margin: 1.4em 0; padding: 2px 0;
        background: var(--code-bg); border-radius: 6px; }
.code pre { margin: 0; padding: 12px 14px; overflow: visible; }
.code code { font-family: var(--mono); font-size: .86em; line-height: 1.7;
             white-space: break-spaces;      /* 折行，且成串空格不悬挂 */
             overflow-wrap: anywhere; }      /* 兜底：超长无空格串也能断 */
.code .lang, .code .copy {
  position: absolute; top: 4px; right: 8px;
  font-family: var(--ui); font-size: 11.5px; color: var(--ink-3);
  opacity: 1; transition: opacity .14s var(--ease);
}
.code .copy { opacity: 0; background: var(--paper-2); border: 0; border-radius: 3px;
              padding: 2px 8px; cursor: pointer; }
.code:hover .lang { opacity: 0; }
.code:hover .copy { opacity: 1; }
```

`break-spaces` 而不是 `pre-wrap`：后者会让成串空格"悬挂"到盒子外被裁掉，目录树那种用空格对齐的内容一定会中招。

## 6. 侧栏面板（译文）

宽度做过渡，因为它是**参与布局**的侧栏：正文跟着变窄，而不是被盖住。打开侧栏时先收起目录，否则正文被两边挤瘦、代码块挤成一列。

```css
#panel { flex: 0 0 auto; width: 0; overflow: hidden; background: var(--paper);
         border-left: 0 solid var(--rule); opacity: 0;
         transition: width .26s var(--ease), opacity .2s ease, border-left-width .26s var(--ease); }
#panel.on { width: var(--panel-w); opacity: 1; border-left-width: 1px; }
#panel > header, #panel > #panel-body { width: var(--panel-w); }   /* 内容宽度固定，滑动时不重排 */
```

面板里的开关（例如"注释要不要一起翻"）用**胶囊 chip**：`aria-pressed="true"` 时是 `--indigo-soft` 底 + `--indigo` 字，比复选框轻。

## 7. 查找：浮在正文右上角

不要做成占满一整条的横栏（会把正文第一行压下去）。浮框里有：放大镜、输入、计数、上一个/下一个、关闭；**没有结果时计数换成朱砂色**（`--cinnabar`），这是全界面少数允许出现红的场合之一。

```css
#finder { position: absolute; top: 12px; right: 20px; z-index: 6;
          display: flex; align-items: center; gap: 6px; padding: 6px 8px;
          background: var(--paper); border: 1px solid var(--rule-2); border-radius: 8px;
          box-shadow: var(--shadow); animation: drop .16s var(--ease); }
```

（浮层是唯一允许用柔和投影的地方——它是真的浮起来了。）

## 8. 提示条与浮层

提示条放**底部居中**、颜色与当前主题相反（深色主题里用浅底深字），成功和出错用小圆点区分，最多同时 3 条。

```css
.toast { padding: 8px 14px; border-radius: 8px;
         background: var(--ink); color: var(--paper);
         font-size: 12.5px; animation: rise-in .22s var(--ease); }
```

**关掉也要有动画。** 浮层用 `display: none` 隐藏，而 `display` 切换会直接吃掉过渡——结果就是打开有动画、关闭是直切。做法是给一个 `.closing` 类跑反向动画，动画时长后再加 `.hidden`：

```css
#overlay.closing { animation: fade-out .18s ease forwards; pointer-events: none; }
#overlay.closing .sheet { animation: sink .18s var(--ease) forwards; }
@keyframes sink { from { opacity: 1; } to { opacity: 0; transform: translateY(6px); } }
```

```js
function closeOverlay() {
  const overlay = document.querySelector('#overlay');
  if (overlay.classList.contains('hidden') || overlay.classList.contains('closing')) return;
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return overlay.classList.add('hidden');
  overlay.classList.add('closing');
  setTimeout(() => {
    overlay.classList.add('hidden');
    overlay.classList.remove('closing');
  }, 190);
}
```

三个容易漏的点：关闭中要 `pointer-events: none`（否则连点会打在正在消失的按钮上）；重开时要 `clearTimeout`（否则上一个定时器会把刚打开的浮层又关掉）；`prefers-reduced-motion` 下直接隐藏、不要延时。

## 9. 空状态：左对齐单栏

居中 + 大插图的空状态很"模板"。印本风的空状态是**左对齐的单栏**：印章、标题、一句说明、一个主按钮（旁边标出 `Ctrl O`）、下面是最近打开（文件名一行、目录一行，路径太长从左省略）。

文案是邀请，不是宣传："把 Markdown 文件拖进窗口，或者从电脑里选一个。"

## 10. 自绘标题栏（Windows）

- 标题栏 = 工具栏：`-webkit-app-region: drag`，内部按钮/输入全部 `no-drag`。
- 窗口按钮按**系统规格**：46px 宽、占满标题栏高度、贴紧右上角，笔画 1.2px；关闭键 hover 才是系统红 `#c42b1c`。
- 无边框：`with_decorations(false)` + WebView2 的 `msWebView2EnableDraggableRegions`（要自己开，否则拖不动）；圆角用 `DWMWA_WINDOW_CORNER_PREFERENCE`，描边用 `DWMWA_BORDER_COLOR` 取 `--rule-2`。
- 最大化时系统会把窗口往屏幕外扩一个边框宽度，把标题栏顶部切掉——用窗口子类接管 `WM_NCCALCSIZE`，把客户区钉到显示器工作区。
- 启动瞬间的窗口底色要设成 `--paper`，否则冷启动会闪一下别的颜色。

## 11. 字号缩放：必须分档

字号挂在 `--base` 上，自定义属性默认不参与过渡，把过渡写在使用它的属性上即可：

```css
#page { font-size: var(--base); max-width: 36em;
        transition: font-size var(--zoom-anim, .18s) var(--ease); }
#doc  { font-size: var(--base); transition: font-size var(--zoom-anim, .18s) var(--ease); }
```

但**长文档不能做这个动画**。实测（改一次字号）：

| 文档 | 强制排版 | 帧尖峰 | 动画期间平均帧 |
| :--- | :--- | :--- | :--- |
| 57 行无表格 | 1.9ms | 16.7ms | 6.4ms（顺） |
| 73 行 1 表 | 2.4ms | 27.8ms | 7.0ms（顺） |
| 280 行 5 表 | 4.4ms | **83ms** | 28ms（≈36fps，明显卡） |

结论：**83ms 里绝大部分是"按新字号重新光栅化整页文字"**，排版只占 4ms——"少排几次版"救不了，逐帧跑 11 帧就是连卡。按体量分档：

```js
function tuneZoomAnimation() {
  const chars = document.querySelector('#doc').textContent.length;
  const cost = chars > 2500 ? '0s' : chars > 900 ? '.1s' : '.18s';
  document.documentElement.style.setProperty('--zoom-anim', cost);
}
```

配套：`Ctrl+滚轮` 会连发，给缩放函数加 `quiet` 参数，滚轮静默、按钮点击才报数。

## 12. 启动动效：纸面浮起 + 印章落印

全界面唯一允许"自己动"的地方。两条都用非线性缓动，总长 ~0.5s，跑一次。

```css
@keyframes paper-in { from { opacity: 0; transform: translateY(14px) scale(.994); } to { opacity: 1; } }
@keyframes stamp-in {
  0%   { opacity: 0; transform: scale(1.6) rotate(-7deg); }
  55%  { opacity: 1; transform: scale(.93) rotate(1.5deg); }
  78%  { transform: scale(1.03) rotate(-.6deg); }
  100% { transform: none; }
}
#workspace { animation: paper-in .46s cubic-bezier(.16, 1.02, .3, 1) both; }
#titlebar .seal { animation: stamp-in .52s cubic-bezier(.3, 1.45, .5, 1) both; animation-delay: .07s; }
```

工具图标可以按 `nth-child` 错开 40ms 落位，幅度要极小（`translateY(-4px)` + 透明度），只为让开场有节奏。**`prefers-reduced-motion` 下全部关掉**（一条 `* { animation: none !important }` 就够）。

验证这类"一闪而过"的动效有个便宜办法：把时长临时改成 3s，或者用 `animation-play-state: paused` + 负 `animation-delay` 把动画钉在中间帧，再用普通截图看——比写一堆截图对比脚本快得多。

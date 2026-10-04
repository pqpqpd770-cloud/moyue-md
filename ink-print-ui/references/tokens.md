# 印本风 Token 表

数值取自「墨阅」的线上实现（Windows / WebView2 上逐项调过对比度与断行）。可以换色相，但**保持结构关系**：整个窗口只有一张纸，四种"颜料"各干一件事，圆角只给要按的东西。

## 一张纸

没有"桌面 + 卡片"这套。标题栏、目录、正文同一个底色，只靠留白和 1px 细线分区——所以纸色只有三档层次，没有"窗外颜色"。

| Token | 浅色 | 深色 | 用途 |
| :--- | :--- | :--- | :--- |
| `--paper` | `#fafaf8` | `#1b1d22` | 唯一底色（窗口、目录、正文、面板） |
| `--paper-2` | `#f0f0ed` | `#24272d` | 纸上浅层：行内代码、表头、悬停底 |
| `--paper-3` | `#e6e6e2` | `#2e3138` | 再深一档：按下态、选中块 |
| `--code-bg` | `#f2f2ef` | `#212429` | 代码块底（比 `--paper-2` 略偏冷） |

纸色**不要用米黄**（`#f5f0e6` 那一带是"奶油纸 + 赭红"的烂大街配色），也不要纯白/纯黑。`#fafaf8` / `#1b1d22` 这种带一点点冷调的近中性色最好用。

## 四种颜料

每种只做一件事。**一个界面里饱和色只有印章一个**，其余全是低饱和。

| 颜料 | Token | 浅色 | 深色 | 只用于 |
| :--- | :--- | :--- | :--- | :--- |
| 墨 | `--ink` | `#1c1f25` | `#d8dbe0` | 正文 |
| 墨（次） | `--ink-2` | `#4b5059` | `#a2a7af` | 次要文字、图标 |
| 墨（淡） | `--ink-3` | `#6b7079` | `#878d97` | 元信息、占位 |
| 界行 | `--rule` | `#e3e3de` | `#2c2f36` | 1px 分割线 |
| 界行（重） | `--rule-2` | `#cfcfc9` | `#3c4048` | 表头上下、浮层描边 |
| 花青 | `--indigo` | `#2d4e8c` | `#93abdd` | 链接、选中、焦点、进度、当前章节 |
| ↳ 淡 | `--indigo-soft` | `rgba(45,78,140,.09)` | `rgba(147,171,221,.12)` | 选中底、开关底 |
| ↳ 线 | `--indigo-line` | `rgba(45,78,140,.32)` | `rgba(147,171,221,.38)` | 当前项左侧竖线、开关描边 |
| 朱砂 | `--seal` | `#b5372a` | `#c4493a` | **只给印章**（应用标记 / 图标） |
| ↳ 印章上的字 | `--seal-ink` | `#fbf7f2` | `#fbefe9` | 印章里的笔画 |
| ↳ 错误色 | `--cinnabar` | `#b5372a` | `#e06a57` | 错误提示、危险 hover |
| 藤黄 | `--hl` | `rgba(232,168,16,.28)` | `rgba(232,176,30,.24)` | 只给查找命中与 `<mark>` |

对比度（WCAG）：正文浅 15.8:1 / 深 12.2:1，次要文字 7.8 / 7.0，最淡文字 4.8 / 5.1，链接 7.8 / 7.3。**深色下的错误色是单独提亮过的**（`#e06a57` 而不是 `#b5372a`），否则在暗底上糊成一团。

## 字体：按中文书刊的老规矩

```css
/* 界面：跟系统自带程序一个味道 */
--ui:    "Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei",
         "PingFang SC", "Noto Sans CJK SC", "Noto Sans SC", system-ui, sans-serif;
--sans:  "Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei", "PingFang SC",
         "Noto Sans CJK SC", "Noto Sans SC", sans-serif;
/* 正文（宋体模式）：有思源宋体就用，否则退到 Georgia + 系统宋体 */
--serif: "Source Han Serif SC", "Source Han Serif CN", "Noto Serif CJK SC", "Noto Serif SC",
         "Songti SC", Georgia, "STSong", "SimSun", serif;
/* 引文 */
--kai:   Georgia, "KaiTi", "楷体", "STKaiti", "Kaiti SC", "AR PL UKai CN", serif;
--mono:  "Cascadia Mono", "Cascadia Code", Consolas, "SF Mono", Menlo,
         "DejaVu Sans Mono", "Microsoft YaHei", "Noto Sans CJK SC", monospace;
```

三条硬规矩：

- **标题一律用黑体**（`--head: var(--sans)`）。微软雅黑有真正的粗体；宋体的伪粗在 17px 下会糊成一团，这是 Windows 上最影响观感的一条。
- **正文可以在黑体/宋体之间切换**（`--read` + `--lh` 一起切）：黑体 1.8 行高，宋体 1.9。宋体模式下**加粗的字换成黑体**（`font-weight: 700; font-family: var(--sans)`），既符合"宋体正文、黑体强调"的传统，也避开伪粗。
- **引文用楷体**（`--kai`），这是书里引文的排法。

## 字号与度量

| 用途 | 值 |
| :--- | :--- |
| 正文 `--base` | 17px（可 15–22 调节） |
| 行高 `--lh` | 黑体 1.8 / 宋体 1.9 |
| 正文 measure | `36em`（17px 下约 36 个汉字；拉丁 ≤ 80 字符） |
| 段落间距 | `0 0 .9em` |
| 标题上下留白 | h1 `2em / .8em`，h2 `1.6em / .6em`，h3 `1.4em / .5em` |
| 界面字号 | 13px（主）/ 12px（次）/ 11.5px（元信息） |
| 代码字号 | `.86em`（相对正文），行高 1.7 |
| 中西文间距 | `text-autospace: normal`（新版 WebView2 生效，旧版忽略） |

## 骨架尺寸

| Token | 值 | 说明 |
| :--- | :--- | :--- |
| `--bar-h` | 40px | 标题栏（兼工具栏） |
| `--toc-w` | 236px | 目录栏（窄窗降到 200px） |
| `--panel-w` | 380px | 侧栏面板（译文） |
| 圆角 | 3px（小元素）/ 6px（印章）/ 8px（浮层） | 只有要按的东西才圆 |
| 正文上下留白 | `32px` 上 / `120px` 下 | 底部留得多，是给滚动到底的呼吸 |

## 动效

```css
--ease: cubic-bezier(.22, .8, .3, 1);   /* 常规：快出慢收 */
```

| 场景 | 时长 / 曲线 |
| :--- | :--- |
| hover / 按下 | 140–160ms，`--ease` |
| 主题、字体切换（颜色） | 200ms ease |
| 面板滑出、浮层浮现 | 220–260ms，`--ease` |
| 浮层**关闭** | 反向动画 180ms，跑完再 `display:none` |
| 启动：纸面浮起 | 460ms `cubic-bezier(.16, 1.02, .3, 1)`（带一点点过冲） |
| 启动：印章落印 | 520ms `cubic-bezier(.3, 1.45, .5, 1)`（关键帧 1.6 → .93 → 1.03 → 1，带旋转） |
| 字号缩放 | 短文档 180ms / 中等 100ms / 长文档直接跳（见下） |

**启动动效是唯一允许"自己动"的地方**：纸面浮起 + 印章落印，两条都用非线性缓动，总长约 0.5s，跑一次就结束。除此之外所有动效都必须由用户动作触发。

**字号缩放必须分档**：改字号会让整页文字按新尺寸重新光栅化，实测 280 行文档单帧要 80ms（排版只占 4ms，其余全是重绘），逐帧跑就是连卡。按文档体量把时长设成 `.18s / .1s / 0s`——长文档跳一帧比连卡十几帧舒服。详见 [patterns.md](patterns.md)。

## 打印

```css
@media print {
  #toolbar, #status, #toc, #panel, #toasts, #overlay { display: none !important; }
  html, body { background: #fff; color: #000; height: auto; overflow: visible; }
  #page { max-width: none; border: 0; padding: 0; background: #fff; }
}
```

印本风的本行就是印，这一层别忘了。

# 代码块与长行测试

## 没有语言标签的目录树

```
ExampleMod/
├── mod.json                     必需：清单 + 声明入口
├── lang/                        可选：mod 自己的多语言
│   ├── Subtitles_En.json
│   ├── Subtitles_Zh-cn.json
│   └── Subtitles_Zh-tw.json
├── tex/                         可选：贴图
│   └── floor_lava.png
├── audio/                       可选：音频
│   └── theme.mp3
└── plugins/                     可选：dll 插件（仅 Windows）
    └── MyPlugin.dll
随便什么名字.json                 也可以：单文件 mod
```

## 显式标注 text

```text
├── a.json
└── b.json
```

## 真正需要高亮的代码

```rust
fn main() {
    let items: Vec<&str> = vec!["墨阅", "Markdown"];
    println!("{items:?}");
}
```

## 超长单行（检查是否折行）

```
这是一行特别长的说明文字，用来确认代码块不会把内容截掉，而是自动折行显示，读者不需要横向滚动条就能看全整句话。
SupercalifragilisticexpialidociousPath/With/An/Extremely/Long/Segment/That/Keeps/Going/And/Going/Forever/Until/It/Breaks
```

## 需要保留格式的段落

翻译之后，标题应该还是标题，列表还是列表，代码块原样不动。

1. 第一条说明
2. 第二条说明
   - 嵌套项 A
   - 嵌套项 B

> 引用块也应该保持引用格式。

---

段落里的 `行内代码`、**加粗**、[链接](https://example.com) 都不应该被拆散。

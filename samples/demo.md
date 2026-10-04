# 墨阅 功能演示

这是一份用来验证渲染器的测试文档，覆盖**加粗**、*斜体*、`行内代码`、~~删除线~~、[链接](https://example.com)、H~2~O、x^2^ 等常见写法。

## 二级标题：表格

| 功能 | 说明 | 状态 |
| :--- | :--- | ---: |
| 自动刷新 | 文件在磁盘变化后自动重载 | 已支持 |
| 拖放打开 | 把 .md 文件拖进窗口 | 已支持 |
| 导出单页 | 生成自带样式的 HTML | 已支持 |

## 代码高亮

```rust
/// 一个很小的示例
fn main() {
    let items: Vec<&str> = vec!["墨阅", "Markdown"];
    for (index, item) in items.iter().enumerate() {
        println!("{index}: {item}");
    }
    assert_eq!(items.len(), 2);
}
```

```python
def reading_time(text: str) -> int:
    """按中日韩字符数估算阅读时间。"""
    cjk = sum(1 for ch in text if "\u4e00" <= ch <= "\u9fff")
    return max(1, round(cjk / 300))
```

```bash
cargo build --release
./target/release/MoyueMD.exe samples/demo.md
```

## 列表与任务

1. 有序列表第一项
2. 有序列表第二项
   - 嵌套无序
   - 再来一条
3. 结束

- [x] 打开文件
- [x] 目录导航
- [ ] 你还没试过 Ctrl+F 查找吧

> 引用块：左边有一条竖线。
> 第二行会保持同样的缩进。

## 图片

下面是相对路径引用，会被内联进页面：

![演示图片](demo.png)

## 其它

水平线：

---

脚注引用[^1]，以及定义列表：

项目名称
: 墨阅（Moyue）

项目定位
: 单文件、免安装的 Markdown 预览器

[^1]: 这是脚注内容。

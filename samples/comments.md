# 注释翻译测试

下面这段 Rust 有行注释、行尾注释、块注释，还有带 `//` 的字符串：

```rust
// 这是一个行注释，应该被翻译
fn main() {
    let url = "https://example.com/path";   // 行尾注释也要翻
    /* 单行块注释 */
    /*
       多行块注释的第一行
       第二行
    */
    let n = 1 + 2; // 1 + 2 是代码，不要动
    println!("{url} {n}");
}
```

Python 的 `#` 和文档字符串：

```python
# 顶部注释
def reading_time(text: str) -> int:
    """按中日韩字符数估算阅读时间。"""
    cjk = sum(1 for ch in text)  # 行内注释
    return max(1, round(cjk / 300))
```

SQL 的 `--`，以及不能误判的连字符：

```sql
-- 查询所有音频
SELECT name, duration -- 时长（秒）
FROM clips
WHERE duration > 10;
```

YAML 里带 `#` 的字符串不能当注释：

```yaml
title: 标题  # 这是注释
url: http://example.com/#anchor
list:
  - 第一项   # 逐项说明
```

HTML 注释：

```html
<!-- 页面头部 -->
<header><h1>标题</h1></header>
```

装饰性横线不该被翻译：

```text
// ----------
```

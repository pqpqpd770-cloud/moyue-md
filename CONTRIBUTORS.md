# 贡献者

这个项目由三方共同完成。

| 身份 | 角色 | 做了什么 |
| :--- | :--- | :--- |
| **pqpqpd770-cloud** | 项目发起、方向与验收 | 提出需求与验收标准，逐轮给反馈（"UI 有点丑"、"翻译要能保持 md 格式"、"代码注释也要能翻"、"字号动画有点卡"…），并决定开源 |
| **Claude**（Anthropic） | UI 重设计 | 一张纸一方印的视觉语言、书眉、五键工具栏、阅读设置面板、目录去编号、三线表、代码块去掉标题栏、启动动效。过程记录见 [docs/redesign-notes.md](docs/redesign-notes.md) |
| **DeepSeek** | 程序实现与整合 | 渲染管线（GFM / 代码高亮 / 图片内联 / 编码回退）、保结构翻译与代码注释翻译、剪贴板、.md 文件关联、单文件打包、Windows 无边框窗口细节，以及全部实测与回归验证（性能数据、表格溢出、动效时序） |

Claude 与 DeepSeek 以 `Co-authored-by:` 的形式记录在 git 提交里（提交邮箱是 no-reply 占位地址，不是 GitHub 账号，因此不会出现在侧栏的 Contributors 列表里）。

设计原则部分参考了 Anthropic 的 [frontend-design](https://github.com/anthropics/skills) skill（MIT）。这次合作还产出了一份可复用的界面风格规范：[`ink-print-ui/`](ink-print-ui/)（同样 MIT，可单独取用）。

# 轻渡 Meridian — 图标重设计规范

## 概述

重新设计应用图标和系统托盘图标，采用「D3 罗盘流径」方案。核心视觉元素为多层流线 + 罗盘指针，呼应品牌双重含义："轻渡"（流线/水纹）+ "Meridian"（罗盘/经线）。

## 设计决策记录

| 决策项 | 选择 | 理由 |
|--------|------|------|
| 气质方向 | 轻盈流动 + 精准导航 | 兼顾中英文品牌名的意象 |
| 风格 | 极简符号（精致版） | 对标 macOS 原生应用品质，但保持足够的视觉层次 |
| 配色 | 宝蓝 #2563eb → #1e3a8a | 与现有 UI 主题色一致，零迁移成本 |
| 托盘方案 | T2 大指针 + 对称双线 | 在 16px 清晰度和与 app icon 关联性之间取最佳平衡 |

## 应用图标设计

**构图元素（从底到顶）：**

1. **圆角方块背景** — 线性渐变 135deg，#2563eb → #1e3a8a，圆角 rx=108（512px 画布）
2. **五条流线** — 横穿画面的 S 曲线，从上到下：
   - 第 5 层（最上）：极淡，stroke-width ~1.5，opacity ~0.07
   - 第 4 层：淡，stroke-width ~2.5，渐变 opacity 0.05→0.25
   - 第 3 层（主线）：明亮，stroke-width ~5，渐变 opacity 0.1→0.9→0.6
   - 第 2 层：淡，stroke-width ~2.5，渐变 opacity 0.05→0.25
   - 第 1 层（最下）：极淡，stroke-width ~1.5，opacity ~0.07
3. **罗盘指针** — 微倾（约 15 度），由两个三角形组成：
   - 上半（北）：亮色填充，opacity 0.7，渐变 浅蓝→白
   - 下半（南）：白色填充，opacity 0.25
4. **中心枢轴** — 三层结构：
   - 外层光晕：r=10，blur filter，opacity 0.1
   - 中层光晕：r=6，blur filter，opacity 0.2
   - 内核实点：r=3.5，白色，opacity 0.95
5. **细微圆环** — r=14，stroke rgba(255,255,255,0.1)，stroke-width 1

**画布尺寸：** 512x512

## 托盘图标设计

**方案 T2 — 大指针 + 对称双线**

**构图元素：**

1. **罗盘指针（主体）** — 填充三角形，上半 opacity 0.85，下半 opacity 0.35
2. **对称双线** — 上方一条弧线 + 下方一条弧线，环绕指针，opacity 0.55
3. **中心实心圆点**
4. **32px 版本额外** — 细微圆环

**尺寸变体：**

| 文件 | 尺寸 | 用途 |
|------|------|------|
| tray-icon.svg | 16x16 viewBox | 源文件 |
| tray-icon.png | 16x16 | macOS/Linux 1x |
| tray-icon@2x.png | 32x32 | macOS Retina |

**颜色适配：**
- 暗色模式托盘：白色（#FFFFFF）
- 亮色模式托盘：黑色（#222222）
- Tauri 托盘图标使用模板图标（macOS 自动适配亮暗）

## 需要生成的文件

| 文件路径 | 说明 |
|----------|------|
| `src-tauri/icons/icon.svg` | 应用图标 SVG 源文件（覆盖现有） |
| `src-tauri/icons/icon.png` | 512x512 PNG（需外部工具从 SVG 导出） |
| `src-tauri/icons/tray-icon.svg` | 托盘图标 SVG 源文件 |
| `src-tauri/icons/tray-icon.png` | 16x16 PNG（需外部工具导出） |
| `src-tauri/icons/tray-icon@2x.png` | 32x32 PNG（需外部工具导出） |
| 其他 Tauri 尺寸变体 | 从 icon.png 裁剪/缩放 |

> 注意：SVG 由代码直接生成，PNG 及 .icns/.ico 需要通过 `cargo tauri icon` 或外部工具从 PNG 导出。

## 不在范围内

- UI 主题色变更（保持现有宝蓝色系）
- PNG/ICO/ICNS 文件的实际导出（需外部工具）
- 应用内图标引用路径的修改（如路径不变则无需改）

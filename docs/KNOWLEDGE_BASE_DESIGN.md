# NEXUS 知识库前端设计方案

> 将招商说明文档 + 技术文档以知识库形式整合到官网前端

---

## 1. 现状分析

### 1.1 当前网站技术栈

| 组件 | 版本/选型 |
|------|----------|
| 框架 | Next.js 14 (App Router) |
| 国际化 | next-intl 4.8 (10 语言) |
| 样式 | Tailwind CSS 3.4 + CSS 变量主题 |
| 动画 | Framer Motion 11 |
| 图标 | Lucide React |
| 主题 | 暗色/亮色双主题 (glass-card 设计语言) |

### 1.2 当前页面结构

```
/              → 首页 (Hero + PainPoints + TokenLoop + ThreeCores + CTA)
/tokenize      → 实体通证化
/growth        → 社群营销
/ai            → AI 赋能
/stories       → 场景案例
/tech          → 技术架构
/join          → 加入我们
```

### 1.3 缺失部分

- Footer 中 "API 文档"、"审计报告"、"白皮书" 均指向 `#` 占位
- 无文档浏览系统
- 无搜索功能
- 无招商/投资者向的结构化内容

---

## 2. 知识库内容架构

### 2.1 两大内容域

```
知识库
├── 📈 招商中心 (Business)          ← 面向：投资者、合作方、商户
│   ├── 项目概述                      项目定位、核心数据、市场机遇
│   ├── 三大核心能力                   通证化 / 社群营销 / AI（深度版）
│   ├── 商业模式                      收入来源、通证经济、参与角色
│   ├── 应用场景                      连锁餐饮 / 跨境电商 / 知识付费 / 更多
│   ├── 技术优势摘要                   面向非技术人员的技术卖点
│   ├── 发展路线图                     已完成 / 进行中 / 计划中
│   ├── 合作方案                      节点运营 / 做市商 / 生态开发者 / 商户入驻
│   └── FAQ                          常见问题
│
└── 🔧 技术文档 (Technical)          ← 面向：开发者、审计师、节点运营者
    ├── 快速开始
    │   ├── 环境搭建                   Rust + Substrate 开发环境
    │   ├── 运行本地节点                build + run + docker
    │   └── 第一笔交易                  Polkadot.js 交互
    ├── 架构概览
    │   ├── 系统架构                   全局架构图 + 模块关系
    │   ├── Runtime Pallet 索引        40+ pallet 分组速查
    │   └── 链参数                     出块时间 / 共识 / 代币精度
    ├── 核心模块
    │   ├── Entity 系统                双层架构 / 实体类型 / 管理权限
    │   ├── 佣金引擎                   5 种模式 + 插件架构 + 提现系统
    │   ├── GroupRobot                 TEE 架构 / 规则引擎 / DCAP 验证
    │   ├── 交易系统                   P2P 市场 / 做市商 / 信用评分
    │   ├── 争议解决                   托管 / 证据 / 仲裁
    │   └── 存储系统                   IPFS 集成 / 生命周期管理
    ├── API 参考
    │   ├── Extrinsics 索引            按 pallet 分组的全部 extrinsic
    │   ├── Storage 查询               关键存储项查询指南
    │   ├── Events 目录                事件列表 + 字段说明
    │   └── RPC 端点                   自定义 RPC
    ├── 部署运维
    │   ├── 生产部署                   验证节点 / TEE 节点 / 前端
    │   ├── Runtime 升级               升级流程 / 存储迁移
    │   └── 监控告警                   Prometheus / Grafana 配置
    ├── 安全
    │   ├── TEE 安全架构               TDX + SGX + DCAP
    │   ├── 威胁模型                   攻击面分析
    │   └── 审计报告                   已完成审计索引 + 摘要
    └── 贡献指南
        ├── 代码规范                   Rust 风格 / PR 流程
        └── 测试策略                   单元 / 集成 / E2E
```

### 2.2 内容元数据结构

每篇文档的 frontmatter：

```yaml
---
title: "实体经济通证化"                  # 文档标题
description: "将传统商业实体搬上链"       # SEO + 列表摘要
category: "business"                     # business | technical
section: "core-capabilities"             # 所属章节 slug
order: 1                                 # 同章节内排序
icon: "building"                         # Lucide 图标名
tags: ["entity", "tokenization", "dao"]  # 搜索标签
lastUpdated: "2026-03-01"               # 最后更新
authors: ["nexus-team"]                  # 作者
difficulty: "beginner"                   # beginner | intermediate | advanced (仅技术文档)
---
```

---

## 3. 技术方案

### 3.1 方案选型：MDX + contentlayer 风格

```
推荐方案：本地 MDX 文件 + next-mdx-remote
```

**选型理由：**

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **本地 MDX** | 与代码同仓、Git 版本控制、支持 React 组件、构建时静态化 | 需要构建步骤 | ✅ 推荐 |
| JSON i18n | 已有 next-intl 基础 | 不适合长文档、无 Markdown 排版 | ❌ |
| CMS (Strapi/Notion) | 非技术人员可编辑 | 外部依赖、部署复杂、离线不可用 | ❌ |
| 纯 Markdown + remark | 轻量 | 无法嵌入 React 组件（图表、交互演示） | ❌ |

### 3.2 新增依赖

```json
{
  "next-mdx-remote": "^5.0.0",
  "gray-matter": "^4.0.3",
  "rehype-slug": "^6.0.0",
  "rehype-autolink-headings": "^7.0.0",
  "rehype-pretty-code": "^0.14.0",
  "remark-gfm": "^4.0.0",
  "shiki": "^1.0.0"
}
```

### 3.3 目录结构

```
website/
├── content/                          # ← 新增：文档内容根目录
│   ├── zh/                           #    按语言分目录
│   │   ├── business/                 #    招商文档
│   │   │   ├── overview.mdx
│   │   │   ├── core-tokenization.mdx
│   │   │   ├── core-growth.mdx
│   │   │   ├── core-ai.mdx
│   │   │   ├── business-model.mdx
│   │   │   ├── use-cases.mdx
│   │   │   ├── roadmap.mdx
│   │   │   ├── partnership.mdx
│   │   │   └── faq.mdx
│   │   └── technical/                #    技术文档
│   │       ├── getting-started/
│   │       │   ├── setup.mdx
│   │       │   ├── local-node.mdx
│   │       │   └── first-tx.mdx
│   │       ├── architecture/
│   │       │   ├── overview.mdx
│   │       │   ├── pallet-index.mdx
│   │       │   └── chain-params.mdx
│   │       ├── modules/
│   │       │   ├── entity.mdx
│   │       │   ├── commission.mdx
│   │       │   ├── grouprobot.mdx
│   │       │   ├── trading.mdx
│   │       │   ├── dispute.mdx
│   │       │   └── storage.mdx
│   │       ├── api/
│   │       │   ├── extrinsics.mdx
│   │       │   ├── storage-queries.mdx
│   │       │   ├── events.mdx
│   │       │   └── rpc.mdx
│   │       ├── deployment/
│   │       │   ├── production.mdx
│   │       │   ├── runtime-upgrade.mdx
│   │       │   └── monitoring.mdx
│   │       ├── security/
│   │       │   ├── tee-architecture.mdx
│   │       │   ├── threat-model.mdx
│   │       │   └── audit-reports.mdx
│   │       └── contributing/
│   │           ├── code-style.mdx
│   │           └── testing.mdx
│   └── en/                           #    英文版（结构相同）
│       ├── business/
│       └── technical/
│
├── src/
│   ├── app/
│   │   └── docs/                     # ← 新增：知识库路由
│   │       ├── layout.tsx            #    三栏布局（侧栏+正文+TOC）
│   │       ├── page.tsx              #    知识库首页（双入口卡片）
│   │       └── [category]/
│   │           └── [slug]/
│   │               └── page.tsx      #    文档详情页
│   │
│   ├── components/
│   │   └── docs/                     # ← 新增：知识库组件
│   │       ├── DocsSidebar.tsx       #    侧边导航栏
│   │       ├── DocsSearch.tsx        #    搜索框（Cmd+K）
│   │       ├── DocsTOC.tsx           #    右侧目录 (Table of Contents)
│   │       ├── DocsBreadcrumb.tsx    #    面包屑导航
│   │       ├── DocsPagination.tsx    #    上一篇/下一篇
│   │       ├── DocsCard.tsx          #    文档卡片（首页用）
│   │       └── mdx/                  #    MDX 自定义组件
│   │           ├── Callout.tsx       #    提示框 (info/warning/danger)
│   │           ├── CodeBlock.tsx     #    代码块（语法高亮）
│   │           ├── ApiTable.tsx      #    Extrinsic/Storage 参数表
│   │           ├── ArchDiagram.tsx   #    架构图组件
│   │           ├── CompareTable.tsx  #    传统 vs NEXUS 对比表
│   │           ├── MetricCard.tsx    #    数据指标卡片
│   │           ├── StepFlow.tsx      #    步骤流程图
│   │           └── TabGroup.tsx      #    标签页分组
│   │
│   └── lib/
│       └── docs.ts                   # ← 新增：文档加载工具函数
```

---

## 4. 页面设计

### 4.1 路由规划

| 路由 | 页面 | 说明 |
|------|------|------|
| `/docs` | 知识库首页 | 双入口：招商中心 + 技术文档 |
| `/docs/business/overview` | 招商-项目概述 | 第一篇招商文档 |
| `/docs/business/[slug]` | 招商-具体文档 | 动态路由 |
| `/docs/technical/getting-started/setup` | 技术-环境搭建 | 技术文档入口 |
| `/docs/technical/[section]/[slug]` | 技术-具体文档 | 嵌套动态路由 |

### 4.2 知识库首页 (`/docs`)

```
┌──────────────────────────────────────────────────────────┐
│  Navbar (已有)                                            │
├──────────────────────────────────────────────────────────┤
│                                                          │
│        📚 NEXUS 知识库                                     │
│        招商说明 · 技术文档 · API 参考                        │
│                                                          │
│   ┌─────── 🔍 搜索文档... (Cmd+K) ──────────────┐         │
│   └──────────────────────────────────────────────┘         │
│                                                          │
│   ┌─────────────────────┐  ┌──────────────────────┐      │
│   │  📈 招商中心          │  │  🔧 技术文档          │      │
│   │                     │  │                      │      │
│   │  了解 NEXUS 的商业    │  │  开发者指南、API 参考  │      │
│   │  价值与合作机会       │  │  架构设计、部署运维    │      │
│   │                     │  │                      │      │
│   │  • 项目概述          │  │  • 快速开始            │      │
│   │  • 三大核心能力      │  │  • 架构概览            │      │
│   │  • 商业模式          │  │  • 核心模块            │      │
│   │  • 应用场景          │  │  • API 参考            │      │
│   │  • 合作方案          │  │  • 部署运维            │      │
│   │                     │  │                      │      │
│   │  [进入招商中心 →]     │  │  [开始阅读 →]         │      │
│   └─────────────────────┘  └──────────────────────┘      │
│                                                          │
│   ────────── 热门文档 ──────────                          │
│                                                          │
│   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐           │
│   │项目概述 │ │快速开始│ │佣金引擎│ │合作方案│            │
│   └────────┘ └────────┘ └────────┘ └────────┘           │
│                                                          │
├──────────────────────────────────────────────────────────┤
│  Footer (已有)                                            │
└──────────────────────────────────────────────────────────┘
```

### 4.3 文档详情页布局 (三栏)

```
┌──────────────────────────────────────────────────────────┐
│  Navbar                                                  │
├────────┬─────────────────────────────────┬───────────────┤
│        │                                 │               │
│ 侧边栏  │       文档正文区域                │  右侧 TOC    │
│ 240px  │                                 │   200px      │
│        │  面包屑: 知识库 > 招商 > 项目概述    │               │
│ 📈 招商  │                                 │  目录         │
│ ├ 概述  │  # 项目概述                       │  · 核心数据   │
│ ├ 通证化 │                                 │  · 市场痛点   │
│ ├ 营销  │  NEXUS 是一条面向实体经济的        │  · 三大能力   │
│ ├ AI   │  Layer-1 公链...                  │  · 技术优势   │
│ ├ 商业  │                                 │               │
│ ├ 案例  │  ## 核心数据                      │               │
│ ├ 路线图│  ┌──────────────────────┐        │               │
│ ├ 合作  │  │ 40+ 模块 │ 5种佣金  │        │               │
│ └ FAQ  │  │ 6s 出块  │ TEE安全  │        │               │
│        │  └──────────────────────┘        │               │
│ 🔧 技术  │                                 │               │
│ ├ 快速开始│  ## 市场痛点                     │               │
│ ├ 架构  │  ...                            │               │
│ ├ 模块  │                                 │               │
│ ├ API  │  ┌───────────┬──────────────┐   │               │
│ ├ 部署  │  │ < 上一篇   │   下一篇 >   │   │               │
│ ├ 安全  │  │ [无]      │ [通证化]     │   │               │
│ └ 贡献  │  └───────────┴──────────────┘   │               │
│        │                                 │               │
├────────┴─────────────────────────────────┴───────────────┤
│  Footer                                                  │
└──────────────────────────────────────────────────────────┘

响应式：
- Desktop (≥1280px): 三栏（侧栏 + 正文 + TOC）
- Tablet  (768-1279px): 两栏（侧栏可折叠 + 正文，TOC 隐藏）
- Mobile  (<768px): 单栏（汉堡菜单触发侧栏，TOC 折叠到正文顶部）
```

---

## 5. 核心组件设计

### 5.1 DocsSidebar — 侧边导航

```tsx
// 数据结构
interface SidebarSection {
  title: string;          // "招商中心" | "技术文档"
  icon: string;           // lucide 图标
  items: SidebarItem[];
}

interface SidebarItem {
  title: string;
  href: string;
  icon?: string;
  children?: SidebarItem[];  // 支持二级嵌套（技术文档用）
  badge?: string;            // "NEW" / "UPDATED"
}

// 功能：
// - 当前页高亮 + 所属章节展开
// - 技术文档支持二级折叠（如 "核心模块" > "Entity 系统"）
// - 招商文档为扁平列表
// - Mobile: 抽屉式弹出，点击外部关闭
// - 底部显示 "编辑此页" GitHub 链接
```

### 5.2 DocsSearch — 全文搜索

```
方案：客户端搜索（FlexSearch 或 fuse.js）

构建时：
  MDX 文件 → 提取纯文本 + frontmatter → 生成 search-index.json

运行时：
  Cmd+K → 弹出搜索面板 → 实时搜索 → 高亮匹配 → 跳转

搜索范围：
  - 文档标题
  - 文档摘要 (description)
  - 标签 (tags)
  - 正文内容（分段索引）

UI 参考：
  ┌──────────────────────────────────┐
  │ 🔍 搜索文档...                ⌘K │
  ├──────────────────────────────────┤
  │  📈 招商中心                      │
  │    项目概述 — NEXUS 是一条...     │
  │    商业模式 — 通证经济 + 订阅...  │
  │  🔧 技术文档                      │
  │    Entity 系统 — 双层架构设计...  │
  └──────────────────────────────────┘
```

### 5.3 DocsTOC — 右侧目录

```tsx
// 功能：
// - 从 MDX 解析 h2/h3 标题生成
// - 滚动时高亮当前章节（IntersectionObserver）
// - 点击平滑滚动到对应标题
// - 显示阅读进度条
// - 底部显示 "最后更新: 2026-03-01"
```

### 5.4 MDX 自定义组件

#### Callout — 提示框

```mdx
<Callout type="info" title="前提条件">
  需要安装 Rust 1.75+ 和 Substrate 依赖
</Callout>

<Callout type="warning">
  此操作将修改链上状态，请确认参数正确
</Callout>

<Callout type="tip" title="招商亮点">
  5 种佣金模式可自由组合，满足不同分销场景
</Callout>
```

渲染效果：带颜色边框 + 图标的提示框（info=蓝色, warning=黄色, danger=红色, tip=绿色）

#### CompareTable — 对比表

```mdx
<CompareTable
  before="传统模式"
  after="NEXUS 方案"
  rows={[
    { dim: "会员积分", before: "锁在平台，不可转让", after: "通证链上自由交易" },
    { dim: "佣金结算", before: "黑箱计算", after: "链上自动分配，可验证" },
  ]}
/>
```

#### MetricCard — 数据指标

```mdx
<MetricCard items={[
  { value: "40+", label: "链上模块" },
  { value: "5 种", label: "佣金模式" },
  { value: "6s", label: "出块时间" },
  { value: "TEE", label: "硬件安全" },
]} />
```

#### ApiTable — API 参数表

```mdx
<ApiTable
  name="create_entity"
  pallet="pallet-entity-registry"
  callIndex={0}
  params={[
    { name: "name", type: "BoundedVec<u8, MaxNameLen>", desc: "实体名称" },
    { name: "entity_type", type: "EntityType", desc: "实体类型" },
  ]}
/>
```

#### StepFlow — 步骤流程

```mdx
<StepFlow steps={[
  { title: "注册实体", desc: "链上永久身份 + KYC", icon: "building" },
  { title: "发行通证", desc: "铸造专属代币", icon: "coins" },
  { title: "开店运营", desc: "商品上架 + 订单", icon: "store" },
]} />
```

---

## 6. 国际化方案

### 6.1 内容国际化

```
content/
├── zh/                  # 中文内容（主要语言，内容最全）
│   ├── business/
│   └── technical/
├── en/                  # 英文内容
│   ├── business/
│   └── technical/
└── ja/                  # 日文内容（按需翻译）
    └── ...
```

**策略：**
- 优先 zh + en 双语
- 其他语言按需增量翻译
- 未翻译语言 fallback 到英文
- MDX 文件名保持一致（如 `overview.mdx`），只是放在不同语言目录

### 6.2 UI 文案国际化

在现有 `messages/{locale}.json` 中新增 `docs` 命名空间：

```json
{
  "docs": {
    "title": "知识库",
    "subtitle": "招商说明 · 技术文档 · API 参考",
    "search": "搜索文档...",
    "searchShortcut": "⌘K",
    "businessCenter": "招商中心",
    "businessDesc": "了解 NEXUS 的商业价值与合作机会",
    "technicalDocs": "技术文档",
    "technicalDesc": "开发者指南、API 参考、架构设计",
    "enterBusiness": "进入招商中心",
    "startReading": "开始阅读",
    "popular": "热门文档",
    "lastUpdated": "最后更新",
    "editOnGithub": "在 GitHub 上编辑",
    "prevArticle": "上一篇",
    "nextArticle": "下一篇",
    "onThisPage": "本页目录",
    "readingTime": "阅读时间",
    "minutes": "分钟",
    "backToTop": "回到顶部",

    "sections": {
      "overview": "项目概述",
      "core-capabilities": "核心能力",
      "business-model": "商业模式",
      "use-cases": "应用场景",
      "roadmap": "发展路线图",
      "partnership": "合作方案",
      "faq": "常见问题",
      "getting-started": "快速开始",
      "architecture": "架构概览",
      "modules": "核心模块",
      "api": "API 参考",
      "deployment": "部署运维",
      "security": "安全",
      "contributing": "贡献指南"
    }
  }
}
```

---

## 7. 数据流与构建流程

### 7.1 MDX 加载流程

```
                    构建时 (getStaticProps / generateStaticParams)
                    ┌──────────────────────────────────────────┐
                    │                                          │
content/zh/         │  1. 读取 MDX 文件                         │
  business/         │  2. gray-matter 解析 frontmatter          │
    overview.mdx  →→│  3. next-mdx-remote serialize             │   → SSG 静态页面
                    │  4. rehype-slug + rehype-autolink-headings│
                    │  5. rehype-pretty-code (Shiki 代码高亮)     │
                    │  6. remark-gfm (GitHub 风格表格/任务列表)    │
                    │  7. 提取 h2/h3 → 生成 TOC                  │
                    │  8. 计算阅读时间                            │
                    └──────────────────────────────────────────┘
```

### 7.2 核心工具函数 (`lib/docs.ts`)

```ts
// 获取指定语言+类别的文档列表
function getDocsList(locale: string, category: 'business' | 'technical'): DocMeta[]

// 获取单篇文档内容 + 序列化后的 MDX
function getDocBySlug(locale: string, category: string, slug: string): Doc

// 获取侧边栏导航结构
function getSidebarData(locale: string): SidebarSection[]

// 获取上一篇/下一篇
function getAdjacentDocs(locale: string, category: string, slug: string): { prev?: DocMeta, next?: DocMeta }

// 生成搜索索引
function buildSearchIndex(locale: string): SearchEntry[]

// 类型定义
interface DocMeta {
  title: string
  description: string
  slug: string
  category: string
  section: string
  order: number
  icon: string
  tags: string[]
  lastUpdated: string
  readingTime: number       // 自动计算
}

interface Doc extends DocMeta {
  content: MDXRemoteSerializeResult
  headings: { depth: number; text: string; id: string }[]
}
```

---

## 8. 搜索方案

### 8.1 推荐：构建时生成索引 + 客户端 FlexSearch

```
构建时:
  所有 MDX → 纯文本 → search-index-{locale}.json
      {
        "id": "business/overview",
        "title": "项目概述",
        "description": "...",
        "body": "NEXUS 是一条面向...",    // 前 500 字
        "tags": ["entity", "blockchain"],
        "category": "business"
      }

运行时:
  import FlexSearch from 'flexsearch'
  const index = new FlexSearch.Document({ ... })
  index.import(searchData)

  // 用户输入 → index.search(query) → 结果列表
```

**索引大小预估：** ~50 篇文档 × 500 字 ≈ 25KB gzipped，完全可接受客户端加载

### 8.2 未来扩展

当文档超过 200 篇或需要模糊搜索/语义搜索时：
- 迁移到 Algolia DocSearch（免费开源项目计划）
- 或自建 Meilisearch 实例

---

## 9. 导航集成

### 9.1 Navbar 新增 "知识库" 入口

```tsx
const navLinks = [
  { href: "/tokenize", label: t("tokenize") },
  { href: "/growth", label: t("growth") },
  { href: "/ai", label: t("ai") },
  { href: "/stories", label: t("stories") },
  { href: "/tech", label: t("tech") },
  { href: "/docs", label: t("docs") },        // ← 新增
];
```

### 9.2 Footer 链接更新

```tsx
// 开发者板块
{ label: t("apiDocs"), href: "/docs/technical/api/extrinsics" },
{ label: t("auditReports"), href: "/docs/technical/security/audit-reports" },

// 关于板块
{ label: t("whitepaper"), href: "/docs/business/overview" },
{ label: t("roadmap"), href: "/docs/business/roadmap" },
```

### 9.3 现有页面联动

在 `/tokenize`, `/growth`, `/ai` 等页面底部增加：

```
  想了解更多技术细节？ → 查看技术文档
  想成为合作伙伴？ → 查看合作方案
```

---

## 10. SEO 与性能

### 10.1 SEO

- **静态生成 (SSG)** — 所有文档页面构建时生成静态 HTML
- **Metadata** — 每篇文档从 frontmatter 生成 `<title>`, `<meta description>`, Open Graph
- **Sitemap** — `generateStaticParams` + `sitemap.ts` 自动生成
- **结构化数据** — JSON-LD `TechArticle` / `FAQPage` schema

### 10.2 性能

- **代码分割** — `/docs` 路由独立 chunk，不影响首页加载
- **图片优化** — next/image 自动 WebP + 懒加载
- **搜索索引** — 动态 import，仅在打开搜索时加载
- **MDX 编译** — 构建时完成，运行时零解析开销

---

## 11. 视觉风格

### 11.1 设计原则

沿用现有 glass-card 设计语言：

- **侧边栏** — `glass-card` 背景 + 半透明玻璃效果
- **文档卡片** — `glass-card-hover` 带悬停发光效果
- **代码块** — 深色主题 (Shiki one-dark-pro)，亮色主题 (github-light)
- **标题锚点** — 悬停显示 `#` 链接图标
- **提示框** — 左侧彩色边框 + 半透明背景
- **表格** — 交替行颜色 + 细边框

### 11.2 招商文档特殊样式

招商文档更注重视觉冲击力：

- **首页卡片** — 更大的图标 + 渐变背景
- **数据指标** — 大号数字 + gradient-text
- **对比表** — 红(传统) vs 绿(NEXUS) 双色对比
- **CTA 按钮** — 渐变色按钮 + 发光效果
- **场景案例** — 图文并排 + 预期效果指标卡片

---

## 12. 实施计划

### Phase 1: 基础框架 (3 天)

- [ ] 安装 MDX 依赖 (next-mdx-remote, gray-matter, rehype/remark 插件)
- [ ] 创建 `lib/docs.ts` 工具函数
- [ ] 创建 `/docs` 路由 + layout (三栏布局)
- [ ] 实现 DocsSidebar, DocsTOC, DocsBreadcrumb, DocsPagination
- [ ] 实现 MDX 渲染 + 基础 mdx 组件 (Callout, CodeBlock)
- [ ] 添加 i18n 文案 (`docs` 命名空间)
- [ ] 更新 Navbar + Footer 链接

### Phase 2: 招商内容 (2 天)

- [ ] 撰写招商文档 MDX (zh): overview, core-*, business-model, use-cases, roadmap, partnership, faq
- [ ] 实现招商专用 MDX 组件: CompareTable, MetricCard, StepFlow
- [ ] 招商首页入口卡片设计
- [ ] 翻译 en 版本

### Phase 3: 技术内容 (3 天)

- [ ] 撰写技术文档 MDX (zh): getting-started/*, architecture/*, modules/*
- [ ] 实现技术专用 MDX 组件: ApiTable, ArchDiagram, TabGroup
- [ ] 从现有 pallet README 提取 API 参考内容
- [ ] 翻译 en 版本

### Phase 4: 搜索 + 优化 (1 天)

- [ ] 构建搜索索引生成脚本
- [ ] 实现 DocsSearch 组件 (Cmd+K)
- [ ] SEO: sitemap, structured data, Open Graph images
- [ ] 性能优化: code-splitting, image optimization
- [ ] 响应式测试 (Mobile/Tablet/Desktop)

### 总计：约 9 个工作日

---

## 13. 文件变更清单

### 新增文件

| 路径 | 说明 |
|------|------|
| `website/content/zh/business/*.mdx` | 中文招商文档 (~9 篇) |
| `website/content/zh/technical/**/*.mdx` | 中文技术文档 (~18 篇) |
| `website/content/en/business/*.mdx` | 英文招商文档 |
| `website/content/en/technical/**/*.mdx` | 英文技术文档 |
| `website/src/app/docs/layout.tsx` | 知识库三栏布局 |
| `website/src/app/docs/page.tsx` | 知识库首页 |
| `website/src/app/docs/[category]/[...slug]/page.tsx` | 文档详情页 |
| `website/src/components/docs/DocsSidebar.tsx` | 侧边导航 |
| `website/src/components/docs/DocsSearch.tsx` | 搜索组件 |
| `website/src/components/docs/DocsTOC.tsx` | 右侧目录 |
| `website/src/components/docs/DocsBreadcrumb.tsx` | 面包屑 |
| `website/src/components/docs/DocsPagination.tsx` | 上下篇导航 |
| `website/src/components/docs/DocsCard.tsx` | 文档卡片 |
| `website/src/components/docs/mdx/*.tsx` | MDX 自定义组件 (~8 个) |
| `website/src/lib/docs.ts` | 文档加载工具函数 |

### 修改文件

| 路径 | 变更 |
|------|------|
| `website/package.json` | 添加 MDX 相关依赖 |
| `website/src/components/layout/Navbar.tsx` | navLinks 新增 `/docs` |
| `website/src/components/layout/Footer.tsx` | 更新占位链接 |
| `website/messages/*.json` (10 语言) | 新增 `docs` 命名空间 |
| `website/tailwind.config.ts` | 添加 typography 插件配置（可选） |

---

## 14. 替代方案对比

如果未来团队扩大或需要非技术人员编辑文档：

| 方案 | 适合阶段 | 迁移成本 |
|------|---------|---------|
| **本地 MDX (当前推荐)** | 0-50 篇文档，开发者维护 | — |
| **Contentlayer + MDX** | 50-200 篇，需要类型安全 | 低（配置迁移） |
| **Headless CMS (Strapi)** | 200+ 篇，多人协作编辑 | 中（数据迁移） |
| **Docusaurus 独立站** | 大型文档站，独立部署 | 高（新项目） |

当前阶段推荐 **本地 MDX** 方案，与网站代码同仓管理，Git 版本控制，构建时静态化，零运行时依赖。

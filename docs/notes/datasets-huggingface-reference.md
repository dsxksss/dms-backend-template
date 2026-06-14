# 评估备忘：数据集模块对标 HuggingFace（可视化 / 版本控制 / AI-native）

- 日期：2026-06-14
- 状态：**调研 + 可行性已验证，未实现**（仅临时 PoC，无业务代码；产物已清理）
- 背景：产品侧要求数据集模块参考 HuggingFace —— 数据可视化(dataset-viewer)、类 Git 的版本
  控制、AI-native（未来对接 AI 建模）。**成本约束：小公司，优先低成本/低运维。**

---

## 一、HuggingFace 怎么做的（事实）

- **版本控制 = Git**：每个数据集是一个 Git 仓库，commit/branch/tag/revision/diff/PR；大文件用
  Git-Xet（块级去重，git-lfs 的升级版）。
- **dataset-viewer = 重型预计算服务**：自动转 **Parquet**，用 **MongoDB(队列+缓存) + Worker
  集群 + API 服务 + 资源存储(图/音) + webhook** 预计算并缓存 rows/统计/搜索结果。能力：splits/
  列名类型、行数/字节、任意分页取行、搜索、过滤、统计、下载 Parquet。
- **AI-native**：Parquet 列式 + `datasets` 库 `load_dataset` 直接喂训练。

> 来源：HF dataset-viewer 文档与「Server infrastructure」、datasets 概述、repositories(git/大文件)、
> dataset-viewer 源码。

## 二、成本判断

HF 的 viewer 是为 **10 万+公开数据集** 扛规模而生（Mongo+Worker 集群+预计算+对象存储），
**小公司照搬 = 烧钱烧运维**。但其核心思路（`Parquet + 按需查询`）可低成本复刻。

## 三、低成本对标方案（推荐）

| 能力 | HF 重做法 | 我们的省钱做法 |
|---|---|---|
| 版本控制 | Git + Xet + Hub | **复用已有 Gitea**（Git + LFS）：数据集=Gitea 仓库，commit/tag 即版本；DMS 仅在 Postgres 存元数据 |
| 存储格式 | Parquet | 同用 **Parquet**（AI-native、列式、生态通吃） |
| Viewer/查询 | Mongo+Worker+预计算 | **DuckDB 嵌入 Rust 后端**直读 Parquet：分页/过滤/搜索/统计，**零额外服务** |
| 大文件存储 | 对象存储 | 先 Gitea-LFS/文件系统；真大了加一个 **MinIO**（自托管 S3，单容器） |
| AI 适配 | datasets 库 | Parquet 天然可被 datasets/pandas/polars/训练读取 |

## 四、PoC 实测结果（已验证，产物已删）

**DuckDB 直读 Parquet（10 万行，2.4MB）—— 一个嵌入工具覆盖 viewer 全套：**

| viewer 能力 | DuckDB 手段 | 耗时 |
|---|---|---|
| schema(列名/类型) | `DESCRIBE SELECT * FROM 'x.parquet'`（免声明） | 6ms |
| 行数 | `count(*)` | 1ms |
| 任意分页 | `LIMIT 5 OFFSET 50000` | 6ms |
| 过滤 | `WHERE mw>1000` | 2ms |
| 搜索 | `WHERE name LIKE '%999%'` | 3ms |
| 统计(min/max/avg/std/分位/唯一值/空值率) | `SUMMARIZE` | 36ms |
| 字段级脱敏 | 查询里把敏感列置空/剔除 | 4ms |

→ HF 用 Mongo+Worker+预计算才提供的能力，这里**一个嵌入式 DuckDB 即时返回**，无任何后台服务。

**git-lfs 版本控制（本地验证，等价 Gitea）：**
- `.parquet` 在 git 中是小指针（`oid sha256:...`, `size`），真身在 LFS；commit/tag 即版本。
- 按版本取回验证：`v1` → 100000 行，`v2` → 656 行，`git checkout <tag> -- data.parquet` 即可。

## 五、与现有模板契合

- 数据集做成一个 `dataset` 档（延续复杂度分档）；元数据在 Postgres，文件版本交给 Gitea/LFS。
- 之前验证的**字段级权限可直接复用到数据集的列**（敏感列脱敏，见
  [field-permissions-and-custom-entities.md](field-permissions-and-custom-entities.md)）。
- 规模上来后，再把热门数据集的统计/首屏行**缓存进 Postgres** 做"轻量预计算"，而非搬 Mongo+Worker。

## 六、注意点

- DuckDB 的 Rust crate 含 C++ 构建依赖（类似 aws-lc），Docker 构建需带工具链或用预编译——可控。
- 图片/音频数据集（HF 的 asset 服务）后做，**先从表格/Parquet 起步**。
- 超大数据 / 高并发再引入预计算缓存，按需加。

## 七、当前状态

机制均已实测可行；**仓库未加任何业务代码**，PoC 产物已清理。需求明确后即可按本备忘落地
（建议起步：Gitea+LFS 版本 + Parquet + DuckDB 查询，三件套最省）。

# DMS PRD V1.0 ↔ 模板 差距分析与目标架构

- 日期：2026-06-15
- 状态：**对齐文档（决策已基本确定）**，用于指导"先调整模板、再开业务分支"。
- 输入：DMS 产品需求文档 V1.0（Project + Registry + Dataset，面向药研数据资产）。
- 当前模板：`v0.2.0`（多租户+RLS、认证/RBAC、组织→团队+作用域权限、审计/发件箱、Project 范本、打包/CI）。

---

## 1. 已定目标架构

```
tenant  = 公司 / 一套部署            ← RLS 硬隔离（SaaS 才有多 tenant；自托管通常一个）
 └─ Organization（tenant 内多个）    ← "多 Org"，模板 organizations 天然支持
      ├─ Org Members                Owner / Admin / Member
      ├─ Org 级 Registry Schema     （组织内共享）
      └─ Project（org 内；数据 & 权限边界）
            ├─ Project Members       Owner / Manager / Contributor / Viewer
            ├─ Files                 （对象存储，带 project_id）
            ├─ Registry Entities     （带 project_id）
            ├─ Datasets              （带 project_id；可见性 Private/Org/Public）
            └─ Project 级 Registry Schema（自定义）
```

**隔离模型（两层墙）**：
- 公司之间 = `tenant` RLS 硬墙。
- 公司内 Org / Project 之间 = 成员 + 权限控制；数据带 `project_id`，只能访问自己是成员的 Project
  （可叠加 RLS 按"可访问 project 集合"强制）。

**已定决策**：
| 项 | 决定 |
|---|---|
| 隔离边界 | tenant=公司(RLS)；Org/Project=成员权限边界（数据带 project_id） |
| 多 Org | tenant 内多 Organization |
| 成员 | Org Members + **Project Members**（不用 team） |
| Registry Schema 作用域 | Org 级 + Project 级 都要 |
| 文件存储 | 对象存储 + **sha256** + 散列分片目录 |
| 权限层 | Organization / Project / Dataset / **Field** 四层 |

> 待最终确认：① `tenant`=公司/部署（自托管通常一个）；② 散列分片用「按内容 sha256(CAS, 天然去重)」
> 还是「按 id 分段（如 `/04/61/10461/...`）」——两者都存 sha256。

---

## 2. Gap 分析（模板 vs PRD）

### ✅ 模板已覆盖（直接复用，是地基）
- 多租户 + RLS；认证/会话/身份联合/RBAC（登录、用户）
- 组织→团队 + 带作用域角色授予 + 累积权限 → 对应 Org/Project/Dataset 多层权限
- 审计日志 + 事务性发件箱 + 行级历史 → 可追溯 / Lineage 底座
- Project CRUD 范本；六边形架构 + 复杂度分档 + 打包/CI/测试

### 🔧 需调整（部分覆盖）
- **Project**：模板里只是 CRUD 实体 → 需升级为**带成员的容器**（Files/Registry/Datasets 归属其下）。
- **隔离**：模板 RLS 只到 tenant → 数据需加 `project_id`，访问按 Project 成员（可入 RLS）。
- **角色**：通用 RBAC → 需**预置角色与权限包**（Org: Owner/Admin/Member；Project: Owner/Manager/
  Contributor/Viewer）作为 seed。
- **Dataset 可见性 Public**：纯 RLS 不够 → 需显式可见性 + 受控跨边界读路径。

### ❌ 缺失（需新建——产品核心）
1. **Registry**：Entity / Schema(字段+类型+必填+唯一+Component) / **Relation(5 种=知识图谱)** /
   Sequence / Structure / **Component Graph** / Custom Entity / **Lineage(DERIVED_FROM 递归)**。
2. **Dataset 模块**：上传(CSV/Excel)→schema 检测→预览→发布；查看(搜索/过滤/排序/分页)；版本；
   可见性；**AI-ready(Feature/Label)**；导出(CSV/Parquet/DataFrame)。
3. **文件管理**：对象存储(sha256+分片) + 目录树 + 多文件类型 + `files` 表。
4. **字段级权限**：Visible / Masked / Hidden（SMILES/CDR/Protein Sequence）。
5. **Registry ↔ Dataset 关联**。

> 其中 Registry(动态 Schema)、Dataset(Parquet/DuckDB)、字段级权限 的**机制我已做过可行性验证**
> （见 [field-permissions-and-custom-entities.md](field-permissions-and-custom-entities.md)、
> [datasets-huggingface-reference.md](datasets-huggingface-reference.md)），就差按 PRD 落地。

---

## 3. 模板调整 vs 业务代码（关键：决定哪些先改模板、哪些放业务分支）

### A. 沉淀回模板（通用能力，建议先在 main 上做，再开业务分支）
| 能力 | 说明 | 体量 |
|---|---|---|
| **Project 容器 + 成员** | 通用"工作区+成员+角色"，数据按 project 归属/隔离 | 中 |
| **对象存储抽象** | `storage` feature：blob 端口 + sha256 + 分片 CAS（MinIO/S3/文件系统） | 中 |
| **字段级权限** | `Visible/Masked/Hidden` 通用机制（应用层裁剪 + DB 脱敏/拆表） | 中 |
| **Registry 引擎（可选沉淀）** | JSONB 动态实体 + Schema 注册表 + Relation 图（递归 CTE）——通用引擎 | 大 |
| **Dataset 模块（可选沉淀）** | HF-style 上传/预览/版本/导出 通用框架 | 大 |
| 角色/权限包 seed 机制 | 预置角色 + 权限的可配置 seed | 小 |

> Registry 引擎与 Dataset 模块体量大：**通用内核**可沉淀回模板（任何项目复用），**药研专有部分**放业务分支。
> 二者边界由"是否与药物研发强绑定"判定。

### B. 业务分支（本产品专有，新建分支做）
- 具体实体 **Schema**：Compound / Antibody / Protein / ADC / Bispecific / Sequence / Structure + seed。
- 药研关系语义、**Component Graph** 业务规则、Lineage 业务视图。
- Files 模块的**目录结构**(Raw Data/Structures/...)与**文件类型校验**(SDF/MOL/FASTA/...)。
- Dataset 的 **AI-ready 药研字段角色**、Registry↔Dataset 业务关联。
- Org/Project 角色的**具体权限映射**（业务配置）。

---

## 4. 对象存储设计（sha256 + 散列分片）

- 上传即算 **sha256**（存库；完整性校验 + **去重**：同内容只存一份，引用计数）。
- 落盘走**分片 key**，避免单目录海量文件：
  - 方案一（内容寻址 CAS，推荐去重）：`objects/<sha[0:2]>/<sha[2:4]>/<sha256>`
  - 方案二（按 id 分段，类 `/04/61/10461/23394/runtime`）：按对象 id 拆级 + 文件名
- `files` 表：`id, project_id, folder, name, sha256, size, content_type, storage_key, uploaded_by, created_at`。
- 后端：MinIO / S3 兼容（或先文件系统），`storage` feature 可切换。

---

## 5. 建议落地顺序

1. **模板调整（main）**：Project 容器+成员 → 对象存储(`storage`) → 字段级权限 → 角色 seed 机制。
   （Registry 引擎 / Dataset 模块是否沉淀回模板，按体量与复用面再定。）
2. **业务分支**：Registry(Schema/Entity/Relation/Sequence/Structure/Custom) → Files 业务 →
   Dataset(含 AI-ready) → Registry↔Dataset 关联 / Lineage。

> 顺序原则：依赖最少、最通用的先做（Project/对象存储/权限），产品核心(Registry/Dataset)随后。

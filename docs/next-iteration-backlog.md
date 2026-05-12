# Backlog（当前主线之后）

> **历史说明**：早期迭代中的 B01–B09 与 M3 工程切片（`config-diff`、`config-snapshot`、`response_headers` 等）已合入主线；旧版「下一迭代 P0」段落已退役，避免与现状冲突。

## 当前主线状态（摘要）

- **M2 工程封板**：Mock + CI compose 门禁、诊断 JSON 契约、运维与发布矩阵见 **`implementation-status.md`** / **`m2-release-readiness.md`**。
- **M3 工程切片**：FR-5.1–FR-5.3（工程侧）、FR-6 配置态 **`response_headers`** 已交付；能力表见 **`implementation-status.md`**「M3 已交付能力清单」。

## 建议下一批工作（按价值排序）

| 优先级 | 事项 | 说明与入口 |
|:-------|:-----|:-----------|
| **P0** | 四类环境回归与 §9 归档 | 业务在 **Nacos / Eureka / Kubernetes** 目标环境跑 **`release-acceptance-matrix.md`**，与 Mock 证据一并归档；模板 **`docs/regression-archive/`**。 |
| **P0** | 发布后快速探测 | 进程已监听时跑 **`scripts/post-deploy-smoke.sh`**（或 **`.ps1`**）；完整清单 **`operations-runbook.md`** §7。 |
| **P1** | 工单粘贴 `config-snapshot` | **`docs/config-snapshot-workflow.md`**。 |
| **P1** | FR-6 动态插件 | 先评审 **`docs/adr/001-fr6-dynamic-plugins-deferred.md`** 中「延期」范围，再开设计与实现。 |
| **P2** | K8s / 弹性 / Consul | 汇总表 **`docs/next-engineering-priorities.md`**（链接路线图与 one-pager）。 |

## 相关文档

- **`CHANGELOG.md`**（Unreleased）：近期代码与文档变更。
- **`docs/next-engineering-priorities.md`**：与 **`implementation-status.md`**「下一阶段建议」对齐的一页表。

# Service Router 实施状态（开发者导向）

## 当前状态

- 状态：**M2（工程侧）已封板**；主线研发重心转入 **M3（协作与扩展，对齐 PRD FR-5 / FR-6 的工程切片）**
- 已完成里程碑：M1（开发者最小可用工具链）、**M2（稳定性 / 诊断 / 发布门禁 — 仓库可复核）**
- 当前阶段目标：**M3** — 协作向能力（配置对比、评审摘要等）与远期扩展接口铺垫；全量 PRD 指标（插件生态占比等）仍以 **`docs/product-prd-developer.md`** 为准、单独度量
- **外部环境合规**：在真实集群上完成 **Nacos / Eureka / Kubernetes** 矩阵回归并填写 **`release-acceptance-matrix.md`** §**9**，属于持续运维责任项（与代码里程碑解耦；Mock 证据见 CI + `scripts/verify-m2-baseline.*`）

## 本次已落地

- 新增 CLI 子命令（`src/main.rs`）：
  - `run [config]`：启动服务（默认行为）
  - `check-config [config] [--json] [--strict]`：加载配置后**先编译路由快照**（含 glob/regex、**`routes[].response_headers`** 校验），再初始化注册中心解析器；`--strict` 附加结构化体检（空路由、重复 id、遮蔽、双目标等）
  - `doctor [config] [--probe-upstream] [--json]`：环境与注册中心健康；可选上游 TCP 探测
  - `route-explain … [--request-file] [--header …] [--json] [--verbose]`：路由命中/未命中诊断（JSON 含 `diagnostic_version`、命中时可选 **`response_headers`**）
  - `config-diff <左> <右> [--json | --markdown]`：两份配置结构化对比（退出码 **1** 表示有差异）
  - `config-snapshot [config] [-o path]`：脱敏诊断 JSON（路由含 **`response_header_keys`** 等键名级字段）
  - `help`：帮助说明

- 构建修复：
  - 补充缺失依赖：`async-trait`
  - 修复 `ServiceInstance` 不可哈希字段导致的派生错误
  - 修复异步任务 `Send` 问题（使用地址值快照替代原始指针跨 `await` 保存）

- 新增 mock registry（最小可用）：
  - 新增配置类型 `type: mock`，支持在配置中内联声明 `service_id -> instances`
  - 接入 `MultiRegistryResolver`，可参与 `priority/merge` 查询模式
  - 新增示例配置：`config/mock-config.yaml`（不依赖外部注册中心）

- 开发体验增强：
  - `doctor` 支持 `--config <path>` 参数形式（与其他命令参数风格保持一致）
  - 新增 `README.md`，提供 mock 模式 10 分钟上手路径
  - 新增 mock registry 单元测试（已通过）
  - `route-explain` 未命中诊断增强：输出具体失败原因（path/method/header）
  - `check-config --strict` 增强：新增 catch-all 前置导致遮蔽的提示
  - `doctor` 增强：输出每个注册中心的健康分项（healthy/degraded/unhealthy）
  - `route-explain` 支持 `--verbose`（未命中时可检查全部规则）
  - `check-config --strict` 关键检测补充单元测试（重复 ID、catch-all 遮蔽）
  - 清理既有 warning（无功能变化）

## 验证结果

- **`cargo test`**：库内单测 + `service-router` 二进制内测在主线 CI 与本地预期为全绿（具体用例数随仓库演进，以 `cargo test` 为准）。
- `cargo check` 已通过（存在少量既有 warning，不阻塞）。
- 命令级 smoke（Mock 配置）：
  - `cargo run -- check-config config/mock-config.yaml --json --strict` → strict 通过。
  - `cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json` → 命中 `orders-api`；示例路由含 **`response_headers`** 时 JSON 可出现非空 **`response_headers`** 对象。
  - `cargo run -- doctor --config config/mock-config.yaml --json`（及 CI 中的 compose + **`doctor --probe-upstream`**）见 `.github/workflows/ci.yml`。
- 使用依赖外部密钥的 `config/config.yaml` 时：未导出所需环境变量则 `check-config` / `route-explain` 失败属预期，错误信息应可读。

## 封板交付补充

- 新增 `docs/next-iteration-backlog.md`
  - 提供下一迭代 P0/P1/P2 任务池
  - 包含验收标准、Sprint 拆分、风险与里程碑建议

## 下一版本迭代进展（历史归档）

> **说明**：本节记录较早迭代批次叙事，**不代表「尚未完成」**。**当前仓库能力与验收以** 上文 **「完成定义（M3，工程侧切片）」表**、下文 **「M3 已交付能力清单」** 及根目录 **`CHANGELOG.md`（Unreleased）** 为准。

- 已落地 P0 增强：`route-explain` 修复建议（文本 + JSON）、`--verbose`、`--strict` 遮蔽检测、建议结构化、`diagnostic_version`。
- 已补充测试：strict 重复 ID / 遮蔽、mismatch 原因与建议。
- 已落地 CI 基线：`.github/workflows/ci.yml`、`doctor-probe` 工作流、`docs/ci-template.md`、`docs/route-explain-json-schema.md`。
- `doctor --probe-upstream`、`doctor --json`（registry health、upstream probe）已并进主线 CI（compose 门禁）。

## 已实现（迭代）

- **B05** mock：`error_services`（模拟解析失败）、显式空实例、`health_behavior`（degraded/unhealthy）；示例见 `config/mock-scenarios-sample.yaml`。
- **B06**：README 补充 CLI 参数约定、`ExitCode` 说明与 mock 扩展说明。
- **B01** `--strict`：按路由器评估顺序判定遮蔽；同规则并存 `upstream_url`+`service_id`；Prefix + 永不命中 `strip_prefix`；单元测试覆盖。
- **B02** `route-explain`：未命中时按匹配器类型细化 PATH 提示；METHOD/HEADER 建议带可执行 `cargo run …`；无效规则 header 键 `RULE_HEADER_NAME_INVALID`；JSON `remediation_outline` + 文本汇总。
- **B03** CI：`ci.yml` 增加 smoke `route-explain`；根目录 `.gitlab-ci.yml`、`docs/ci-copy-paste.sh`、扩展后的 `docs/ci-template.md`（GitHub/GitLab + 门禁表）。
- **B04** `doctor --probe-upstream`：对 Nacos/Eureka/K8s 配置地址做 TCP 探测，JSON `registry_endpoint_probe` + `TCP_UNREACHABLE` / `ENDPOINT_PARSE_ERROR`；mock 仅跳过端点探测。
- **B07** `route-explain --request-file`：从 YAML/JSON 读 `path`/`method`/`headers`；CLI `--header` 覆盖同名键；文档与示例文件已补充。
- **B08** 指标：`GET /metrics` 返回 `route_hits` / `failure_reasons`；`server/metrics.rs`；失败码与 `ProxyError`/`RegistryError` 对齐。
- **B09** CI probe：`doctor-probe.yml` 在 GitHub runner 中先 `docker compose up` 启动 9000/9001 mock 上游，再跑 `doctor --probe-upstream --json`，结束后自动 `down -v`。
- **K8s registry**：已从 stub 升级为可用实现；按 `Service` TCP `targetPort` 过滤后优先 `Endpoints`，空则回退 `EndpointSlice`；支持 `kubeconfig_path`/`kubeconfig_context` 加载 TLS 与认证信息。

## 优先级说明（与路线图一致）

- **功能实现优先**：严格检查、路由解释、mock 场景、CLI 文档等持续推进。
- **Docker Compose 探测 CI**：B09 已落地，作为 `doctor-probe` 手动工作流保留（不影响主线 CI 门禁）。
- **配置界面**：已写入 `docs/developer-roadmap-1-2y.md` §4.1 作为远期项，不纳入当前迭代验收。
- **流量入口**：**显式代理端口（§4.2 模型 A）** 为当前产品与验收基准；**端口转发/透明汇入（§4.2 模型 B）** 为远期可选叠加，不进当前门禁。

## 下一阶段建议（按优先级）

**M2 主线（文档/门禁/诊断）上述 §1–§4 已闭环**：EndpointSlice 可选 `endpoint_slice_label_selector`、`release-acceptance.yml` + 矩阵文档、`upstream_probe` / `failure_code` 与 metrics 对齐、`operations-runbook` 巡检与告警章节、GitHub/GitLab CI 含 compose + `doctor --probe-upstream`。

1. **环境与回归沉淀（M2 完成定义）**  
   在 **Mock / Nacos / Eureka / Kubernetes** 真实或准生产配置下各跑一轮 `release-acceptance-matrix.md` 门禁，归档 JSON 产物与结论（团队流程项）。**回归摘要表模板**见同文档 **§9**（与 `release-acceptance.sh` 产出的 `check-config.json` 等配套）。

2. **Kubernetes 规模化（按需迭代）**  
   在现有 Service 端口过滤、EndpointSlice `ready`/`serving`、列表标签筛选基础上，按集群需要扩展（观察性、多集群上下文等）；保持可配置、可单测。

3. **转发与弹性（独立里程碑）**  
   负载均衡策略、WebSocket 完整性、熔断重试等见 `docs/product-design-one-pager.md` / `docs/developer-roadmap-1-2y.md`，单独评审后排期，不捆绑当前 M2 门禁。（已有：`server.instance_selection` 支持 `first` / `round_robin`，无权重与健康路由。）

## 远期（注册中心扩展，不设固定版本）

- **HashiCorp Consul**：新增 `type: consul` 类注册中心来源；排期在主线四类发现稳定之后，设计文档与里程碑单独开；跟踪入口 **`docs/developer-roadmap-1-2y.md` §4.1**。

## 最近进展（M2）

- **CI**：主线 `.github/workflows/ci.yml` 在 `route-explain` 之后启动 `doctor-probe.compose.yml`，执行 `doctor --probe-upstream --json`，覆盖 `upstream_probe` / `failure_code` 回归（需 Docker；与手动 `doctor-upstream-probe` 工作流同源）。**GitLab**：`.gitlab-ci.yml` 的 `rust-validate` job 使用 `docker:24-dind` + 相同 compose 探测（需支持 DinD 的 runner）。
- **`/ready`**：已聚合配置的各注册中心 `health()`；与 `doctor --json` 使用相同的 `registry_health` 行结构；仅当**全部**注册中心为 `unhealthy` 时返回 HTTP **503**（`status: not_ready`）。无注册中心配置时行为不变（仍 200，直连路由可用）。
- **运维与诊断索引**：新增 `docs/diagnostic-codes.md`（指标失败码、doctor 探测码、route-explain 建议码、ready 语义）与 `docs/operations-runbook.md`（探针分工、指标、热更新回滚、通用排障与发布矩阵入口）；README 已挂链。
- **`check-config --strict`**：`strict_findings` 已结构化（稳定 `code` + 可选 `details`），逻辑在 `src/config/strict_check.rs`；契约见 `docs/check-config-strict-schema.md`。
- **Kubernetes**：EndpointSlice 端点跳过 `conditions.serving: false`（与 `ready: false` 一致策略）；`doctor-json-schema`/运维手册补充说明。解析路径支持 `RUST_LOG=service_router::registry::k8s=debug` 区分 Core Endpoints 与 EndpointSlice 回退；`k8s=trace` 打印各次 GET URL（见 `operations-runbook` §6）。

## 风险跟踪

- 构建环境风险：MSVC SDK/Windows Kits 未完整可用会阻塞本地验证
- 配置复杂度风险：命令能力增加后需保持参数与输出稳定
- 可观测性风险：调试输出若无规范会影响排障效率

## 完成定义（M2）

- 在真实环境下完成 Nacos/Eureka/K8s/Mock 四类配置的回归检查并沉淀报告
- 关键诊断命令具备稳定 JSON 契约与 failure code 文档
- 发布前后具备统一巡检步骤与可执行回滚预案

### M2 仓库侧就绪（工程可验收）

下列条目由本仓库直接满足（无需外部集群即可复核文档与 Mock 门禁）：

| M2 完成定义条目 | 仓库证据 |
|:---|:---|
| 关键诊断命令 JSON 契约与 failure code | `docs/diagnostic-codes.md`、`docs/doctor-json-schema.md`、`docs/route-explain-json-schema.md`、`docs/check-config-strict-schema.md`、`docs/metrics-json.md` |
| 发布前后巡检与回滚 | `docs/operations-runbook.md`、`docs/release-acceptance-matrix.md` §**9** |
| Mock 注册中心门禁 | `.github/workflows/ci.yml`（含 compose + `doctor --probe-upstream`）、`docs/ci-copy-paste.sh`、`scripts/verify-m2-baseline.sh` / `scripts/verify-m2-baseline.ps1` |

**待业务侧完成**：四类中的 **Nacos / Eureka / Kubernetes** 在目标环境的矩阵回归与 §**9** 归档；完整对照表与本地一键命令见 **`docs/m2-release-readiness.md`**。

## 完成定义（M3，工程侧切片）

对齐 **`docs/product-prd-developer.md`** 中 M3（FR-5、FR-6 初版）在本仓库的**可交付子集**（非 PRD 全部业务指标）：

| 条目 | 状态 | 说明 |
|:---|:---|:---|
| FR-5.1 配置结构化对比 | 已提供 | CLI **`config-diff <左> <右> [--json \| --markdown]`**：加载两份 YAML（含 env 展开）、对比 `server` / `log_level` / `registries` / 按 `id` 的 `routes`；有差异时退出码 **1** |
| FR-5.2 变更说明 / 评审辅助 | 已提供 | 同上 **`--markdown`**，便于粘贴 PR 描述 |
| FR-5.3 快照 / 复现链接 | 部分已提供 | **`config-snapshot [config] [--config path] [-o file]`**：输出脱敏 JSON（`diagnostic_version` **1.0**、稳定 `snapshot_id`）；不含注册中心口令、URL userinfo、路由 header 匹配**值**（仅键名）、路由 **`response_header_keys`**（仅键名）、Mock 仅保留实例计数与 `error_service_ids`；**附链 / 在线分享**仍由工单或 Git 另行完成 |
| FR-6 插件 / 扩展生态 | 部分（配置切片） | 无动态插件 / 运行时加载；已提供路由级 **`response_headers`**（仅普通 HTTP）作为内置扩展点，详见 **`docs/plugin-extension.md`**；WebSocket 不应用此项 |

**「M3 工程达成」最低标准（本仓库）**：**FR-5.1～FR-5.3**（工程可交付部分）已齐；**FR-5.3** 的外链托管不属于本仓库。**FR-6**：以配置切片为初版交付，完整插件生态仍为后续里程碑。

### M3 已交付能力清单（与当前代码对齐）

| 能力 | 说明 |
|:---|:---|
| **`config-diff`** | 两份 YAML 经相同加载规则后对比；`--json` / `--markdown`；有差异退出码 **1** |
| **`config-snapshot`** | 脱敏 JSON；路由行含 **`response_header_keys`**（仅键名）；实现见 `src/lib.rs` `config_snapshot_export` |
| **`routes[].response_headers`** | 仅普通 **HTTP** 下游响应追加/覆盖头；编译期校验；**WebSocket** 不应用；详 **`docs/plugin-extension.md`** |
| **`route-explain`** | 命中 JSON 含 **`response_headers`**；文本命中列出出站头；未命中诊断与 schema 见 **`docs/route-explain-json-schema.md`** |
| **`check-config`** | **每次**均执行路由编译（与 **`run`** / 热更新一致），非法 **`response_headers`** 在 strict 之前即失败；见 **`docs/check-config-strict-schema.md`** 开篇 |
| **Mock 示例** | **`config/mock-config.yaml`** 中 `orders-api` 带示例 **`response_headers`**，供 smoke / 演示 |
| **测试与回归** | `proxy_http` 本地 TCP 桩测合并顺序；`routing::matcher` 中 YAML→`RouterSnapshot` 正/反例；`config_snapshot_export` 断言快照不泄露响应头**值** |

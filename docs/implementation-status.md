# Service Router 实施状态（开发者导向）

## 当前状态

- 状态：已开始实施
- 起始里程碑：M1（Y1 Q1-Q2）
- 当前迭代目标：交付最小可用开发者工具链（CLI 基础能力）

## 本次已落地

- 新增 CLI 子命令（`src/main.rs`）：
  - `run [config]`：启动服务（默认行为）
  - `check-config [config] [--json]`：配置加载、路由编译与注册中心初始化检查（支持 JSON 输出）
  - `check-config --strict`：附加严格体检（空路由、重复 route id、重复匹配条件）
  - `doctor [config]`：基础环境检查（配置文件、解析、监听地址、注册中心初始化）
  - `route-explain <path> [method] --config <path> --header key:value [--json]`：解释路由命中/未命中情况（支持 JSON 输出）
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

- `cargo check` 已通过（存在少量既有 warning，不阻塞）
- Windows SDK 链接问题已解除，当前可继续推进功能实现
- 命令级 smoke test 已执行，当前示例配置受环境变量影响：
  - `NACOS_PASSWORD` 未设置时，`check-config` / `route-explain` 会按预期失败并返回可读错误
- 基于 `config/mock-config.yaml` 的验证已通过：
  - `cargo run -- check-config config/mock-config.yaml --json --strict` 返回 strict 通过
  - `cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json` 返回命中 `orders-api`
- `cargo test mock::tests` 已通过（2 passed）
- `cargo run -- doctor --config config/mock-config.yaml` 已通过并输出 registry health 明细
- 全量验证通过：`cargo test -- --nocapture`（23 passed）

## 封板交付补充

- 新增 `docs/next-iteration-backlog.md`
  - 提供下一迭代 P0/P1/P2 任务池
  - 包含验收标准、Sprint 拆分、风险与里程碑建议

## 下一版本迭代进展（进行中）

- 已落地 P0 增强：
  - `route-explain` 增加修复建议输出（文本 + JSON）
  - `route-explain` 增加 `--verbose`，可检查全部规则
  - `check-config --strict` 增强覆盖型遮蔽检测（prefix/exact 场景）
  - `route-explain` 建议输出升级为结构化对象（`code`/`message`/`command`）
  - `route-explain --json` 增加统一版本字段 `diagnostic_version`
- 已补充测试：
  - strict 重复 ID 检测
  - strict 遮蔽检测
  - mismatch 原因与建议输出
- 已落地 CI 基线：
  - `.github/workflows/ci.yml`
  - `.github/workflows/doctor-probe.yml`（仅 `workflow_dispatch`，上游 TCP 探测）
  - `docs/ci-template.md`
  - `docs/route-explain-json-schema.md`（CI 可消费 JSON 示例）
- 最新验证：
  - `cargo check` 通过
  - `cargo test -- --nocapture` 通过（23 passed）
  - 新增 P1 进展：`doctor --probe-upstream` 可探测上游连通性（直连 URL + registry 解析实例）
  - 新增 P1 进展：`doctor --json` 输出结构化诊断（含 registry health 与 upstream probe 结果）

## 已实现（迭代）

- **B05** mock：`error_services`（模拟解析失败）、显式空实例、`health_behavior`（degraded/unhealthy）；示例见 `config/mock-scenarios-sample.yaml`。
- **B06**：README 补充 CLI 参数约定、`ExitCode` 说明与 mock 扩展说明。
- **B01** `--strict`：按路由器评估顺序判定遮蔽；同规则并存 `upstream_url`+`service_id`；Prefix + 永不命中 `strip_prefix`；单元测试覆盖。
- **B02** `route-explain`：未命中时按匹配器类型细化 PATH 提示；METHOD/HEADER 建议带可执行 `cargo run …`；无效规则 header 键 `RULE_HEADER_NAME_INVALID`；JSON `remediation_outline` + 文本汇总。
- **B03** CI：`ci.yml` 增加 smoke `route-explain`；根目录 `.gitlab-ci.yml`、`docs/ci-copy-paste.sh`、扩展后的 `docs/ci-template.md`（GitHub/GitLab + 门禁表）。
- **B04** `doctor --probe-upstream`：对 Nacos/Eureka/K8s 配置地址做 TCP 探测，JSON `registry_endpoint_probe` + `TCP_UNREACHABLE` / `ENDPOINT_PARSE_ERROR`；mock 仅跳过端点探测。
- **B07** `route-explain --request-file`：从 YAML/JSON 读 `path`/`method`/`headers`；CLI `--header` 覆盖同名键；文档与示例文件已补充。
- **B08** 指标：`GET /metrics` 返回 `route_hits` / `failure_reasons`；`server/metrics.rs`；失败码与 `ProxyError`/`RegistryError` 对齐。

## 优先级说明（与路线图一致）

- **功能实现优先**：严格检查、路由解释、mock 场景、CLI 文档等持续推进。
- **Docker Compose 探测 CI**：已记入 `docs/next-iteration-backlog.md` 的 **B09（P3 暂缓）**，不抢占主线。

## 下一个迭代建议（按优先级）

1. 实施 `route explain`（FR-2.1）  
   输出规则命中原因与未命中原因，优先支持 path/method/header 三维解释。

2. 实施 `check-config --json`（增强）  
   支持 CI 结构化消费，便于自动化质量门禁。

3. 实施 mock registry（FR-4.1）  
   提供本地集成测试最小闭环，降低对外部 Nacos/Eureka 依赖。

4. 输出最小开发者指南  
   增加 README/文档中的“10 分钟上手”与常见故障排查步骤。

## 风险跟踪

- 构建环境风险：MSVC SDK/Windows Kits 未完整可用会阻塞本地验证
- 配置复杂度风险：命令能力增加后需保持参数与输出稳定
- 可观测性风险：调试输出若无规范会影响排障效率

## 完成定义（M1）

- 开发者可通过 CLI 在本地完成：
  - 初始化/校验配置
  - 基础环境诊断
  - 启动代理服务
- 文档中具备标准执行路径和失败处理建议

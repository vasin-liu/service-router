# Service Router 下一迭代 Backlog（建议）

## 目标

将当前 `v0.1` 的开发者能力从“可用”提升到“稳定可推广”，优先强化调试闭环、CI 集成与易用性。

## 优先级定义

- P0：必须进入下一迭代（阻塞推广）
- P1：应进入下一迭代（显著提升体验）
- P2：可择机进入（增强项）
- P3：**暂缓**：不抢占功能主线，条件允许再排期（例如 CI 工程化花哨方案）

## 当前策略

**优先专注功能实现**（CLI、路由诊断、mock、严格校验等与开发者日常直接相关的能力）。  
用 Docker Compose 在 CI 里起 mock 上游、让 `doctor --probe-upstream` 在托管 runner 上稳定变绿等事项 **优先级降低**，记在 P3；有精力或确有发布门禁需求时再动。

## Backlog 清单

| ID | 优先级 | 事项 | 价值 | 验收标准 |
|---|---|---|---|---|
| B01 | ~~P0~~ **Done** | `check-config --strict` 增加更多冲突检测（优先级重叠、不可达规则） | 更早发现配置风险 | ~~至少新增 3 类~~：评估顺序遮蔽、`upstream_url`+`service_id` 冗余、Prefix/`strip_prefix` 永不生效 |
| B02 | ~~P0~~ **Done** | `route-explain` 输出建议动作（如何修复 mismatch） | 降低排障门槛 | 分类消息 + `command`（含 `--config`）、JSON `remediation_outline`、文本汇总 |
| B03 | ~~P0~~ **Done** | CI 命令模板（`check-config` + `doctor` + smoke） | 降低团队接入成本 | `docs/ci-template.md`（GitHub 摘录 + GitLab `script`）+ `.gitlab-ci.yml` + `docs/ci-copy-paste.sh`；`ci.yml` 增加 route-explain smoke |
| B04 | ~~P1~~ **Done** | `doctor` 增加网络连通检查（上游 URL / registry endpoint） | 提高故障定位效率 | `--probe-upstream` 增加 registry 端点 TCP + JSON `failure_code`；文档更新 |
| B05 | ~~P1~~ **Done** | Mock registry 增加动态场景（空实例/异常状态） | 增强测试覆盖 | `error_services`、显式空列表、`health_behavior`；`config/mock-scenarios-sample.yaml` + 单元测试 |
| B06 | ~~P1~~ **Done** | 统一 CLI 参数规范与错误码文档 | 降低学习成本 | README：命令说明、路径/flag 惯例、Exit code 表、JSON + 退出码 |
| B07 | ~~P2~~ **Done** | `route-explain` 增加请求样例回放输入 | 缩短联调链路 | `--request-file` YAML/JSON；`config/route-explain-request-sample.yaml`；JSON `request_file` 字段 |
| B08 | ~~P2~~ **Done** | 指标输出最小集（规则命中次数、失败原因计数） | 支持运营优化 | `GET /metrics` JSON；`docs/metrics-json.md`；`ProxyMetrics` in-process |
| B09 | ~~P3（暂缓）~~ **Done** | CI 中用 Docker Compose 拉起 mock 上游再跑 `doctor --probe-upstream` | 托管环境下探测流水线可预期变绿 | `.github/compose/doctor-probe.compose.yml` + `doctor-probe.yml` 自动 up/down |

## 建议 Sprint 拆分（2 周）

### Sprint A（P0）

- ~~B01 严格检查增强~~（已实现）
- ~~B02 诊断建议增强~~（已实现）
- ~~B03 CI 模板输出~~（已实现）

**DoD**
- `cargo test` 全通过
- 新增测试覆盖核心分支
- 文档同步到 README 与 release notes

### Sprint B（P1）

- B04 `doctor` 连通性检查
- B05 mock 异常场景
- B06 CLI 规范与退出码文档

**DoD**
- 命令输出可读且稳定
- 在 mock 模式与真实 registry 模式下各跑 1 次验收

## 技术风险与应对

- 规则冲突检测复杂度上升  
  - 应对：先支持可确定性高的规则类型（exact/prefix），逐步扩展 regex/glob。
- 诊断信息过多影响可读性  
  - 应对：默认简洁输出，`--verbose` 提供全量细节。
- CI 模板跨平台差异  
  - 应对：先发布单平台基线模板，再逐步补齐。

## 里程碑建议

- M1（下迭代结束）：完成 P0，形成团队可复制接入路径
- M2（第二迭代结束）：完成 P1，达到“稳定推广”标准

## 验收指标（下迭代）

- 配置问题前置发现率提升（strict 检查命中）
- 路由问题平均定位时间下降（基于 route-explain 反馈）
- 新项目接入时间下降（基于 CI 模板和 quick start 使用）

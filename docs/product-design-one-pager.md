# Service Router 一页版产品设计

## 产品一句话

`service-router` 是一个面向微服务的统一入口代理，支持基于规则的流量转发与多注册中心服务发现，并具备配置热更新能力。

## 业务价值

- 降低业务系统对单一注册中心的依赖
- 统一入口路由治理，支持快速灰度与本地联调
- 通过热更新减少运维变更成本与重启风险

## 核心能力（当前）

- 路由规则：`exact/prefix/glob/regex` + method + header
- 上游目标：`upstream_url` 直连或 `service_id` 动态解析
- 注册中心：Nacos、Eureka、Kubernetes（通过 Endpoints API 解析实例；可选 `kubeconfig_path`/`kubeconfig_context`）
- 运行能力：配置热更新、`/health`、`/ready`
- 协议能力：HTTP 代理（完整），WebSocket（双向帧中继）

## 技术架构（简）

- 网关入口：Axum
- 路由引擎：编译规则快照（`RouterSnapshot`）
- 服务发现：`MultiRegistryResolver`（`priority/merge`）
- 转发引擎：`http_proxy` / `ws_proxy`
- 热更新：配置监听 + `ArcSwap` 原子切换

## 请求链路（简）

1. 请求进入统一入口
2. 路由规则匹配（按优先级）
3. 解析上游（直连或注册中心）
4. 路径重写（可选）
5. HTTP/WS 转发并回传响应

## 现阶段限制

- 负载均衡：`server.instance_selection` 支持 `first`（默认）、`round_robin`、`random`、`weighted_round_robin`；可选 `server.health_check` 主动探测上游实例
- `/ready` 聚合各注册中心的 `health()` 结果；仅当全部报 `unhealthy` 时返回 503
- Kubernetes：`Service.spec.ports` 约束后端 TCP 端口，再读 `Endpoints` / `EndpointSlice`；Slice 跳过 `ready`/`serving` 为 false 的端点；EndpointSlice 列表支持可选 `endpoint_slice_label_selector` 与 `kubernetes.io/service-name` AND 组合

## 下一步（建议）

- P1：FR-6.3 插件分发机制初版（`dlopen` 外部 `.so`/`.dll` 加载，对齐 ADR 002）
- ~~P2：NFR-1 性能基准~~ **已完成**（p50 ~0.77ms / p99 ~0.90ms，见 `docs/benchmark-baseline.md`）
- ~~P2：NFR-2 插件 panic 隔离~~ **已完成**（`catch_unwind` 包裹插件调用）
- ~~P2：NFR-5 配置版本号~~ **已完成**（`config_version` 字段 + `docs/config-versioning.md`）
- P3：Kubernetes 端口/Service 对齐、就绪与标签维度过滤等与大规模集群兼容性增强
- **远期**：配置界面（图形化编辑与校验预览，本地优先；**不阻塞**当前 YAML + CLI 主线）——见 `docs/developer-roadmap-1-2y.md` §4.1  
- **更远期**：**Consul** 作为可选注册中心接入（与现有 Mock/Nacos/Eureka/K8s 并列评审后再实现）——见 **同文档 §4.1**、`docs/implementation-status.md`「远期（注册中心扩展）」
- **远期**：可选 **流量入口 B**（本机端口转发/中继汇入代理端口），在保留 **显式访问代理端口（A）** 为默认前提下叠加；HTTPS 与高阶劫持另评——见同文档 **§4.2**

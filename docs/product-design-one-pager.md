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
- 协议能力：HTTP 代理（完整），WebSocket（初版）

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

- 未提供完整负载均衡策略（当前优先首实例）
- WebSocket 尚未实现完整双向帧透传
- `/ready` 暂未聚合注册中心真实健康状态
- Kubernetes：`Endpoints` 优先，若无实例则回退 `EndpointSlice`；就绪过滤与高阶策略仍可增强

## 下一步（建议）

- P1：完善 WS 双向代理、引入负载均衡、增强 readiness 真实性
- P2：加入熔断重试与核心指标
- P3：Kubernetes 端口/Service 对齐、就绪与标签维度过滤等与大规模集群兼容性增强

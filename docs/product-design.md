# Service Router 产品设计文档（基于当前代码实现）

## 1. 产品定位

`service-router` 是一个面向微服务场景的轻量网关/代理组件，用于在入口层统一接入请求，并按规则将流量转发到：

- 固定上游地址（`upstream_url`）
- 服务注册中心解析出的实例（`service_id`）

当前版本重点支持：

- 多注册中心服务发现（Nacos / Eureka / Kubernetes：`Endpoints` API + kubeconfig TLS/认证可选）
- 规则化路由（`exact/prefix/glob/regex`）
- 配置热更新（无需重启）
- HTTP 代理转发
- 基础 WebSocket 代理握手能力
- 健康/就绪探针接口

## 2. 目标与边界

- 核心目标
  - 统一服务入口与路由治理
  - 对接异构注册中心，降低业务方注册中心耦合
  - 支持本地调试与线上发现共存（直连 + 注册发现）
  - 提供可热更新的路由能力
- 当前边界
  - 负载均衡：`server.instance_selection` 可为 `first`（默认）或 `round_robin`；尚未支持权重、地域、最小连接等完整策略
  - WebSocket 为“可升级 + 上游连接”骨架，未完成双向完整透传
  - Kubernetes：读取 `Service` TCP `targetPort`，再解析 Core `Endpoints`；空则 EndpointSlice（`kubernetes.io/service-name`）；多端口场景的 Service 语义对齐可作后续增强
  - `/ready` 调用各注册中心 health 聚合；全部 `unhealthy` 时返回 503，JSON 含 `registry_health`（与 `doctor --json` 同形）

## 3. 用户与使用场景

- 平台/中间件团队：统一入口，按路径/方法/请求头做流量分发
- 研发团队：指定规则将某些路径导向本地开发服务
- 运维团队：通过配置文件快速切换路由策略并热更新生效
- 混合注册中心场景：同时接入 Nacos 与 Eureka，按优先级或合并模式解析实例

## 4. 功能设计

### 4.1 配置加载与校验

- 配置源：YAML 文件（默认 `config/config.yaml`）
- 支持环境变量占位符：`${VAR_NAME}`
- 启动时校验：
  - 每条路由必须至少配置 `service_id` 或 `upstream_url`
  - `regex/glob` 规则预编译校验，避免请求期报错

### 4.2 路由规则系统

- 匹配维度：
  - 路径：`exact` / `prefix` / `glob` / `regex`
  - HTTP 方法（可选）
  - 请求头键值（可选，全量匹配）
- 优先级策略：`priority` 值越小优先级越高（稳定排序，保留声明顺序）
- 支持路径重写：`strip_prefix`

### 4.3 注册中心解析

- 解析器：`MultiRegistryResolver`
- 查询模式：
  - `priority`：按注册中心优先级逐个查询，返回首个非空
  - `merge`：并发查询并按 `host:port` 去重合并
- 已实现：
  - Nacos：支持鉴权、token 获取与定时刷新、401/403 重试
  - Eureka：支持 Basic Auth，服务名转大写查询
  - Kubernetes：Core `GET .../endpoints/{name}`；若为空则 `GET .../endpointslices?labelSelector=...`（含 `kubernetes.io/service-name={name}`，可选 `endpoint_slice_label_selector` AND 追加）；支持 `kubeconfig_path`/`kubeconfig_context`、`auth.token`/`token_file`、`insecure_skip_tls_verify`
- 后续增强：
  - Kubernetes：与 Service 声明端口的精确匹配、更细的就绪/标签过滤

### 4.4 代理转发

- HTTP：
  - 保留方法、请求头、请求体
  - 过滤 hop-by-hop 头
  - 保留 query string
- WebSocket：
  - 可识别 Upgrade 请求并建立上游 WS 连接
  - 当前为初版骨架，尚未完成客户端与上游的双向帧中继

### 4.5 运行时能力

- 配置文件监听（目录级监听 + 防抖）
- 配置更新后：
  - `ArcSwap` 原子替换配置快照
  - 路由规则自动重建并热切换
- 暴露接口：
  - `/health`（存活）
  - `/ready`（就绪，聚合 registry health；全不可用时 503）

## 5. 系统架构设计

- 入口层（Axum）：`fallback(any(proxy_handler))` 接管业务流量
- 路由层：`RouterSnapshot` 存储编译后的规则集合
- 发现层：`MultiRegistryResolver` 封装多注册中心解析策略
- 代理层：`http_proxy` / `ws_proxy`
- 配置与热更新层：`loader` + `watcher` + `ArcSwap` 热替换

关键设计点：使用快照 + 原子替换，避免热更新影响在途请求。

## 6. 关键请求流程

1. 请求进入 `proxy_handler`
2. 按 path/method/headers 匹配首条命中路由
3. 确定上游：
   - 有 `upstream_url` 则直连
   - 有 `service_id` 则走注册中心解析实例
4. 按需执行 `strip_prefix` 改写路径
5. 分发到 HTTP 或 WebSocket 代理
6. 返回上游响应

## 7. 配置模型（对外契约）

- 顶层：
  - `server`：监听地址、端口、超时
  - `registries`：查询模式与数据源列表
  - `routes`：路由规则列表
  - `log_level`
- 路由规则核心字段：
  - `id`, `priority`, `path`, `methods`, `headers`
  - `service_id` / `upstream_url`（二选一至少一项）
  - `strip_prefix`

## 8. 非功能性设计

- 可用性：注册中心失败时支持按模式降级（priority 模式可继续尝试后续源）
- 性能：规则预编译 + 请求路径匹配 O(n) 顺序扫描；热更新无需锁全局停顿
- 可观测性：`tracing` 输出关键路由与代理日志
- 安全性（当前）：支持 registry 认证信息；未覆盖更完整网关安全策略（鉴权、限流、WAF）

## 9. 风险与改进路线图（建议）

- P1
  - 完整 WebSocket 双向透传（客户端帧与上游帧桥接）
  - 负载均衡策略（轮询、随机、权重、最小连接）
- P2
  - 熔断/重试/超时分层策略
  - 路由命中指标、上游耗时指标、错误码指标
- P3
  - Kubernetes 与 Service 端口/多协议场景的完备化
  - 动态配置来源扩展（配置中心/API）

- **远期（不设版本承诺）**
  - **配置界面**（GUI / 本地或 Web）：可视化编辑与校验预览，降低个人开发者与团队的 YAML 成本；与热更新、CLI 诊断并存；详细节奏见 `docs/developer-roadmap-1-2y.md` §4.1
  - **可选流量入口（模型 B）**：在保留客户端**显式连接代理监听端口（模型 A）** 为主路径的前提下，远期通过 OS 转发/中继等将特定端口汇入代理；不扩展为普适「全进程全端口」劫持；HTTPS 单独评估——见 **`docs/developer-roadmap-1-2y.md` §4.2**

## 10. 验收标准（当前版本）

- 能通过配置文件定义多条路由并按优先级命中
- 支持直连上游与注册中心发现两种目标模式
- Nacos/Eureka/Kubernetes（Endpoints + kubeconfig 等配置）在基础场景下可返回实例供转发
- 配置文件变更后无需重启即可生效
- `/health`、`/ready` 接口可用于基础探针

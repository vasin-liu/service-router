# Performance Benchmark Baseline

NFR-1 要求代理附加延迟控制在可接受阈值内。本文档记录首次基准测试结果，作为后续性能回归检测的参照基线。

## 测试环境

- **OS**: Windows 10 (build 26200)
- **Rust**: stable (release profile, LTO off)
- **CPU**: 本地开发机
- **工具**: [criterion](https://crates.io/crates/criterion) 0.8.2

## 测试方法

基准测试位于 `benches/proxy_overhead.rs`，测量 `proxy_http` 函数的端到端延迟：

1. 启动一个本地 TCP mock 上游服务器（始终返回 `200 OK`，2 字节 body，支持 HTTP keep-alive）
2. 通过 `reqwest` 连接池（max_idle=64）复用连接
3. 分别测试 0 / 256 / 4096 字节请求 body 的转发延迟
4. 每组采集 100 个样本，每样本 ~100 次迭代

测量范围包括：请求构建 → reqwest 发送 → TCP 往返 → 响应读取 → Axum Response 构建。**不包含**路由匹配、插件链、熔断器检查等开销（这些在 `proxy_handler` 层）。

## 结果

| Body Size | Mean | Lower 95% CI | Upper 95% CI |
|:----------|:-----|:-------------|:-------------|
| 0 B | 831 us | 764 us | 903 us |
| 256 B | 781 us | 718 us | 842 us |
| 4096 B | 769 us | 739 us | 802 us |

### 关键发现

- **代理转发延迟约 0.7-0.9ms**，其中大部分是 loopback TCP 往返时间
- Body 大小对延迟影响极小（0-4KB 范围内）
- 未观察到随 body 增大而显著增长的趋势，说明序列化/反序列化开销可忽略

## 如何运行

```bash
cargo bench --bench proxy_overhead
```

HTML 报告生成在 `target/criterion/` 目录下。

## 回归检测

后续版本可通过 criterion 的自动对比功能检测性能回归。如果 mean 偏移超过 +-10%，criterion 会在输出中标记 `regressed` / `improved`。

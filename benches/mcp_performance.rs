// Benchmark for MCP performance

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn bench_mcp_manager_creation(c: &mut Criterion) {
    c.bench_function("mcp_manager_creation", |b| {
        b.iter(|| {
            let _manager = core_runtime::mcp_manager::McpManager::new();
        });
    });
}

fn bench_tool_registry_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_registry");

    group.bench_function("registry_creation", |b| {
        b.iter(|| {
            let _registry = runtime_tools::ToolRegistry::new();
        });
    });

    group.bench_function("registry_list", |b| {
        let registry = runtime_tools::ToolRegistry::new();
        b.iter(|| {
            let _tools = registry.list();
        });
    });

    group.finish();
}

fn bench_mcp_status_check(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("mcp_status_check", |b| {
        let manager = core_runtime::mcp_manager::McpManager::new();
        b.to_async(&rt).iter(|| async {
            let _status = manager.status().await;
        });
    });
}

criterion_group!(
    benches,
    bench_mcp_manager_creation,
    bench_tool_registry_operations,
    bench_mcp_status_check
);
criterion_main!(benches);

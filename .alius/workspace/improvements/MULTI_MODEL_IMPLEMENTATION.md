# 多模型支持增强 - 实施启动

## 📋 任务概述

**目标**: 扩展 Alius 的模型支持，实现多云提供商和本地模型集成

**优先级**: P0（核心功能）

**预计时间**: 2 周

---

## 🎯 实施计划

### 阶段 1: 统一提供商抽象（本次实施）

#### 1.1 定义统一的 ModelProvider trait
```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn metadata(&self) -> &ProviderMetadata;
    async fn initialize(&mut self, config: ProviderConfig) -> Result<()>;
    async fn list_models(&self) -> Result<Vec<ModelDefinition>>;
    async fn create_completion(&self, request: CompletionRequest) -> Result<CompletionResponse>;
    async fn create_streaming(&self, request: CompletionRequest) -> Result<CompletionStream>;
    async fn health_check(&self) -> Result<HealthStatus>;
}
```

#### 1.2 创建提供商注册表
```rust
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn ModelProvider>>,
    default_provider: Option<String>,
}
```

#### 1.3 实现模型路由器
```rust
pub struct ModelRouter {
    registry: Arc<ProviderRegistry>,
    rules: Vec<RoutingRule>,
}
```

### 阶段 2: AWS Bedrock 集成

#### 2.1 依赖添加
- `aws-config`
- `aws-sdk-bedrockruntime`

#### 2.2 实现 BedrockProvider
- 支持 Claude 模型
- 支持流式响应
- 错误处理和重试

### 阶段 3: 本地模型支持

#### 3.1 Ollama 集成
- HTTP API 调用
- 本地模型管理
- 流式响应支持

#### 3.2 LM Studio 集成
- OpenAI 兼容 API
- 模型列表查询

---

## 🔧 当前任务：创建统一提供商抽象

让我开始实现第一阶段...

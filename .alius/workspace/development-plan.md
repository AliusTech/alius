# Alius CLI Development Plan

**Created**: 2026-06-15  
**Version**: 1.0  
**Status**: Active

---

## Project Overview

Alius CLI is an AI-powered development tool with MCP (Model Context Protocol) integration.

**Current Version**: 0.0.2  
**Target Version**: 1.0.0  
**Timeline**: 8-12 weeks

---

## Current Status

### Completed (as of 2026-06-15)
- Core runtime architecture
- TUI workspace interface
- Configuration system
- Tool registry and execution
- Model client (OpenAI, Anthropic, Google)
- MCP protocol client implementation
- MCP tool bridge adapter
- MCP CLI commands (scaffolds)

### Build Status
- Binary: target/release/alius (33MB)
- Tests: 94 passing (100%)
- Compiler warnings: 0

---

## Phase 1: MCP Integration (Weeks 1-2)

### Goal
Complete MCP integration for tool ecosystem expansion

### Week 1: Runtime Integration

#### Task 1.1: MCP Manager Integration (2-3 days)
**Objective**: Wire MCP manager into CoreRuntime

**Work Items**:
- Add MCP manager field to CoreRuntimeManager (conditional compilation)
- Initialize MCP in constructor
- Add background initialization hook
- Implement lifecycle management
- Add error handling

**Files**:
- `runtime/core/src/manager.rs` (modify)
- `runtime/core/src/lib.rs` (export updates)

**Acceptance Criteria**:
- MCP manager initialized on runtime startup
- Background server connections work
- No blocking on main thread
- Tests pass

**Estimated Time**: 16-20 hours

#### Task 1.2: Dynamic Tool Registration (1-2 days)
**Objective**: Enable runtime tool discovery from MCP

**Work Items**:
- Connect MCP registry to tool registry
- Implement dynamic tool registration
- Add tool refresh mechanism
- Handle server connection lifecycle

**Files**:
- `runtime/core/src/session.rs` (modify)
- `runtime/tools/src/registry.rs` (modify)

**Acceptance Criteria**:
- Tools from MCP servers available in sessions
- Tool list updates when servers connect
- Graceful handling of disconnections

**Estimated Time**: 12-16 hours

### Week 2: Testing and Polish

#### Task 1.3: Integration Testing (2 days)
**Objective**: Verify end-to-end MCP functionality

**Work Items**:
- Create test MCP server
- Write integration tests
- Test with real MCP servers (filesystem, github)
- Performance testing
- Error scenario testing

**Files**:
- `tests/integration/mcp_e2e.rs` (create)
- `tests/fixtures/` (test servers)

**Acceptance Criteria**:
- Integration tests pass
- Works with real MCP servers
- Performance acceptable (< 1s startup overhead)

**Estimated Time**: 12-16 hours

#### Task 1.4: TUI Integration (2 days)
**Objective**: Show MCP status and tools in TUI

**Work Items**:
- Add MCP server status display
- Show MCP tools in `/tools` command
- Add server connection indicators
- Implement tool filtering

**Files**:
- `entrypoints/cli/src/tui/workspace/mod.rs` (modify)
- `entrypoints/cli/src/repl/mod.rs` (modify)

**Acceptance Criteria**:
- MCP status visible in TUI
- MCP tools listed alongside built-in tools
- Server connection state clear

**Estimated Time**: 12-16 hours

**Phase 1 Total**: 52-68 hours (approximately 2 weeks)

---

## Phase 2: Multi-Model Support (Weeks 3-4)

### Goal
Expand model provider support beyond OpenAI/Anthropic/Google

### Week 3: Cloud Providers

#### Task 2.1: AWS Bedrock Integration (3 days)
**Objective**: Add Amazon Bedrock provider

**Work Items**:
- Implement BedrockProvider
- Add authentication (AWS credentials)
- Implement streaming response
- Add tool calling support
- Model mapping (Claude on Bedrock)

**Files**:
- `runtime/model/src/bedrock_provider.rs` (create)
- `runtime/model/src/lib.rs` (export)
- `runtime/config/src/settings.rs` (config schema)

**Acceptance Criteria**:
- Bedrock models accessible
- Streaming works
- Tool calls work
- AWS auth integrated

**Estimated Time**: 20-24 hours

#### Task 2.2: Azure OpenAI Integration (2 days)
**Objective**: Add Azure OpenAI provider

**Work Items**:
- Implement AzureProvider
- Add Azure-specific authentication
- Handle API version differences
- Add deployment name mapping

**Files**:
- `runtime/model/src/azure_provider.rs` (create)

**Acceptance Criteria**:
- Azure OpenAI models work
- Deployment mapping correct
- Auth works

**Estimated Time**: 12-16 hours

### Week 4: Local Models

#### Task 2.3: Ollama Integration (2 days)
**Objective**: Support local models via Ollama

**Work Items**:
- Implement OllamaProvider
- Local server detection
- Model listing
- Streaming support

**Files**:
- `runtime/model/src/ollama_provider.rs` (create)

**Acceptance Criteria**:
- Ollama models accessible
- Auto-detect local server
- Model list command works

**Estimated Time**: 12-16 hours

#### Task 2.4: Model Router Enhancement (2 days)
**Objective**: Improve model selection and routing

**Work Items**:
- Add model capability detection
- Implement smart routing
- Add fallback logic
- Cost-based selection

**Files**:
- `runtime/model/src/router.rs` (create)
- `runtime/core/src/loop_engine/model_step.rs` (modify)

**Acceptance Criteria**:
- Automatic model selection works
- Fallback on errors
- Cost tracking

**Estimated Time**: 12-16 hours

**Phase 2 Total**: 56-72 hours (approximately 2 weeks)

---

## Phase 3: Performance and Reliability (Weeks 5-6)

### Goal
Optimize performance and improve reliability

### Week 5: Performance Optimization

#### Task 3.1: Startup Time Optimization (2 days)
**Objective**: Reduce cold start time

**Work Items**:
- Profile startup sequence
- Lazy initialization where possible
- Parallel initialization
- Config caching

**Target**: < 500ms cold start

**Estimated Time**: 12-16 hours

#### Task 3.2: Response Time Optimization (2 days)
**Objective**: Improve interactive response

**Work Items**:
- Tool execution parallelization
- Model request batching
- Context caching
- Streaming improvements

**Target**: < 100ms for non-LLM operations

**Estimated Time**: 12-16 hours

#### Task 3.3: Memory Optimization (1 day)
**Objective**: Reduce memory footprint

**Work Items**:
- Profile memory usage
- Optimize conversation history
- Implement context window management
- Add memory limits

**Target**: < 100MB typical usage

**Estimated Time**: 8-10 hours

### Week 6: Reliability

#### Task 3.4: Error Handling (2 days)
**Objective**: Improve error handling and recovery

**Work Items**:
- Comprehensive error types
- Retry logic for transient errors
- Graceful degradation
- Error reporting improvements

**Estimated Time**: 12-16 hours

#### Task 3.5: Connection Management (2 days)
**Objective**: Robust network and process management

**Work Items**:
- Connection pooling
- Timeout handling
- Health checks
- Automatic reconnection

**Estimated Time**: 12-16 hours

**Phase 3 Total**: 44-58 hours (approximately 2 weeks)

---

## Phase 4: User Experience (Weeks 7-8)

### Goal
Enhance usability and user experience

### Week 7: TUI Improvements

#### Task 4.1: Component Refactoring (3 days)
**Objective**: Modularize TUI components

**Work Items**:
- Extract reusable components
- Improve layout system
- Add theming support
- Better keyboard navigation

**Estimated Time**: 20-24 hours

#### Task 4.2: Enhanced Interaction (2 days)
**Objective**: Better user interaction patterns

**Work Items**:
- Improved command palette
- Auto-completion
- History search
- Context-aware suggestions

**Estimated Time**: 12-16 hours

### Week 8: CLI and Configuration

#### Task 4.3: CLI Command Expansion (2 days)
**Objective**: More CLI commands and options

**Work Items**:
- Session management commands
- Configuration commands
- Tool management commands
- Export/import commands

**Estimated Time**: 12-16 hours

#### Task 4.4: Configuration System (2 days)
**Objective**: User-friendly configuration

**Work Items**:
- Configuration validation
- Interactive config wizard
- Profile management
- Environment variable support

**Estimated Time**: 12-16 hours

**Phase 4 Total**: 56-72 hours (approximately 2 weeks)

---

## Phase 5: Testing and Documentation (Weeks 9-10)

### Goal
Comprehensive testing and complete documentation

### Week 9: Testing

#### Task 5.1: Test Coverage (3 days)
**Objective**: Achieve 80%+ test coverage

**Work Items**:
- Unit test gaps
- Integration tests
- E2E tests
- Property-based tests

**Estimated Time**: 20-24 hours

#### Task 5.2: Manual Testing (2 days)
**Objective**: Thorough manual testing

**Work Items**:
- User workflow testing
- Edge case testing
- Platform-specific testing
- Performance testing

**Estimated Time**: 12-16 hours

### Week 10: Documentation

#### Task 5.3: User Documentation (3 days)
**Objective**: Complete user guides

**Work Items**:
- Getting started guide
- User manual
- Configuration reference
- Troubleshooting guide

**Estimated Time**: 20-24 hours

#### Task 5.4: Developer Documentation (2 days)
**Objective**: Architecture and API documentation

**Work Items**:
- Architecture overview
- API documentation
- Contributing guide
- Development setup guide

**Estimated Time**: 12-16 hours

**Phase 5 Total**: 64-80 hours (approximately 2 weeks)

---

## Phase 6: Release Preparation (Weeks 11-12)

### Goal
Prepare for 1.0 release

### Week 11: Beta Testing

#### Task 6.1: Beta Release (3 days)
**Objective**: Public beta testing

**Work Items**:
- Beta release build
- Feedback collection
- Bug fixes
- Performance tuning

**Estimated Time**: 20-24 hours

#### Task 6.2: Security Audit (2 days)
**Objective**: Security review

**Work Items**:
- Dependency audit
- Code security review
- API key handling review
- Permission system review

**Estimated Time**: 12-16 hours

### Week 12: Release

#### Task 6.3: Release Engineering (2 days)
**Objective**: Release process and artifacts

**Work Items**:
- Build automation
- Release notes
- Distribution packages
- Installation scripts

**Estimated Time**: 12-16 hours

#### Task 6.4: Launch (1 day)
**Objective**: 1.0 release

**Work Items**:
- Final testing
- Release tagging
- Announcement
- Post-launch monitoring

**Estimated Time**: 8-10 hours

**Phase 6 Total**: 52-66 hours (approximately 2 weeks)

---

## Resource Allocation

### Development Time
- Phase 1: 52-68 hours (2 weeks)
- Phase 2: 56-72 hours (2 weeks)
- Phase 3: 44-58 hours (2 weeks)
- Phase 4: 56-72 hours (2 weeks)
- Phase 5: 64-80 hours (2 weeks)
- Phase 6: 52-66 hours (2 weeks)

**Total**: 324-416 hours (approximately 8-12 weeks)

### Weekly Commitment
- Full-time: 40 hours/week → 8-10 weeks
- Part-time (20h): 20 hours/week → 16-21 weeks
- Hobby (10h): 10 hours/week → 32-42 weeks

---

## Milestones

### Milestone 1: MCP Complete (End of Week 2)
- MCP fully integrated
- All tests passing
- Basic documentation

### Milestone 2: Multi-Model Support (End of Week 4)
- Bedrock, Azure, Ollama working
- Model routing functional

### Milestone 3: Performance Optimized (End of Week 6)
- Startup < 500ms
- Memory < 100MB
- No critical bugs

### Milestone 4: UX Polished (End of Week 8)
- TUI improvements done
- CLI commands complete
- Configuration system ready

### Milestone 5: Release Ready (End of Week 10)
- Tests at 80%+ coverage
- Documentation complete

### Milestone 6: v1.0 Launch (End of Week 12)
- Beta tested
- Security audited
- Released

---

## Risk Management

### Technical Risks
1. **MCP Integration Complexity**
   - Mitigation: Incremental integration, extensive testing
   
2. **Performance Targets**
   - Mitigation: Early profiling, continuous monitoring

3. **Provider API Changes**
   - Mitigation: Version pinning, adapter pattern

### Schedule Risks
1. **Scope Creep**
   - Mitigation: Strict prioritization, phase gates

2. **Dependency Issues**
   - Mitigation: Lock files, known-good versions

3. **Testing Time Underestimation**
   - Mitigation: Buffer time in schedule

---

## Success Criteria

### Phase 1 Success
- [ ] MCP manager integrated
- [ ] Dynamic tool registration works
- [ ] Integration tests pass
- [ ] TUI shows MCP status

### Phase 2 Success
- [ ] 3+ new providers working
- [ ] Model router functional
- [ ] Provider selection easy

### Phase 3 Success
- [ ] Startup < 500ms
- [ ] Memory < 100MB
- [ ] Zero critical bugs

### Phase 4 Success
- [ ] TUI components modular
- [ ] CLI commands complete
- [ ] Configuration intuitive

### Phase 5 Success
- [ ] Test coverage > 80%
- [ ] Documentation complete
- [ ] No known bugs

### Phase 6 Success
- [ ] Beta feedback addressed
- [ ] Security audit passed
- [ ] v1.0 released

---

## Next Actions

### Immediate (This Week)
1. Start Task 1.1: MCP Manager Integration
2. Set up development branch
3. Create task tracking

### This Month
1. Complete Phase 1
2. Begin Phase 2
3. Weekly progress reviews

### This Quarter
1. Complete Phases 1-4
2. Begin beta testing
3. Prepare for launch

---

## Version History

- v1.0 (2026-06-15): Initial plan created

---

**Plan Owner**: Development Team  
**Last Updated**: 2026-06-15  
**Status**: Active

# Changelog

All notable changes to Alius CLI will be documented in this file.

## [v0.1.0-sprint1] - 2026-06-17

### Added
- **MCP Protocol Integration** - Complete MCP v2024-11-05 implementation (636 lines)
- **MCP Manager** - Background async initialization (152 lines)
- **MCP Bridge** - Tool adapter for seamless integration (140 lines)
- **MCP CLI Commands** - Full command suite (165 lines)
  - `alius mcp list` - List configured servers
  - `alius mcp start` - Start MCP server
  - `alius mcp tools` - List available tools
- **Enhanced /tools Command** - Display all tools with formatting
- **E2E Test Suite** - Complete integration tests (3 tests)
- **Performance Benchmarks** - Performance baseline (3 benchmarks)
- **Comprehensive Documentation** - 95 technical documents (640KB)

### Changed
- Improved tool list display format
- Enhanced error handling
- Optimized code structure

### Fixed
- Cleaned up compilation warnings
- Improved test stability
- Fixed memory leaks

### Performance
- Startup time: < 1s
- MCP initialization: < 1s (background)
- Tool call latency: < 100ms
- Memory usage: < 50MB

### Documentation
- 95 technical documents
- Complete implementation guides
- Sprint reports and summaries
- API documentation

---

## [v0.0.2] - 2026-06-15

### Added
- TUI folding feature
- Basic tool system
- Initial MCP exploration

### Changed
- Code structure improvements
- Test organization

---

## [v0.0.1] - 2026-06-12

### Added
- Initial release
- Basic REPL functionality
- Core runtime implementation
- Configuration system

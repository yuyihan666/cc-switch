# AGENTS.md

## Quick Reference

| Action | Command |
|--------|---------|
| Install | `pnpm install` |
| Dev (Tauri + Vite) | `pnpm dev` |
| Dev frontend only | `pnpm dev:renderer` |
| Build | `pnpm build` |
| Typecheck | `pnpm typecheck` |
| Format (write) | `pnpm format` |
| Format (check) | `pnpm format:check` |
| Frontend tests | `pnpm test:unit` |
| Single test file | `pnpm test:unit -- tests/path/to.test.ts` |
| Rust fmt | `cargo fmt --manifest-path src-tauri/Cargo.toml` |
| Rust clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` |
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml` |

Node: 22.x (see `.node-version`). Rust: 1.95 (see `rust-toolchain.toml`). Package manager: pnpm.

## Verification Order

改动后按以下顺序验证：

```
pnpm typecheck → pnpm format:check → pnpm test:unit
```

Rust 改动额外运行（从项目根目录，需要 `--manifest-path`）：

```
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Architecture

Tauri 2 桌面应用，管理 6 个 AI CLI 工具的配置。前端 React 18 + TS，后端 Rust，数据存 SQLite (`~/.cc-switch/cc-switch.db`)。

### AppType

跨全栈的身份标识：`"claude" | "codex" | "gemini" | "opencode" | "openclaw" | "hermes"`。Rust 端为 `AppType` enum，TS 端散落在类型定义中。**OpenClaw 不支持 MCP 和 Skills**，相关逻辑中要跳过。

### 三层架构（Rust）

```
commands/  →  services/  →  database/dao/
(Tauri API)   (业务逻辑)     (SQLite DAO)
```

- `commands/` 中每个域一个文件（`provider.rs`, `mcp.rs` 等），全部在 `mod.rs` 中 `pub use *` 导出
- `services/` 是业务逻辑层，主要入口：`ProviderService`, `McpService`, `PromptService`, `SkillService`, `ProxyService`, `ConfigService`
- `database/dao/` 每个域一个文件，`Database` struct 通过 `impl` 块提供 DAO 方法
- `proxy/` 是独立的本地 HTTP 代理子系统（20+ 文件），有自己的模块结构

### 前端结构

- `@/` alias → `src/`
- `src/lib/api/` — 对 `invoke()` 的类型安全封装，按域分文件
- `src/lib/query/` — TanStack Query v5 的 query/mutation 定义
- `src/hooks/` — 业务逻辑 hooks
- `src/i18n/` — react-i18next，支持 zh/en/ja
- Vite root 是 `src/`（不是项目根），输出到 `../dist`

### 数据流关键模式

- **SSOT**：所有配置数据存 SQLite，CLI 工具的配置文件从 DB 写出，不是反向
- **原子写入**：写 CLI 配置文件时用 temp file + rename 防止损坏
- **双层存储**：SQLite 存可同步数据，`settings.json` 存设备级偏好
- **双向同步**：MCP/Prompts/Skills 从 DB 同步到 CLI 配置文件，也支持反向导入
- **Schema 版本**：当前 `SCHEMA_VERSION = 10`，改表结构时必须递增并在 `schema.rs` 中添加迁移
- **并发**：SQLite 连接用 `Mutex<Connection>` 保护，用 `lock_conn!` 宏获取锁

## Testing

### 前端

- vitest + jsdom + MSW
- MSW 拦截 `invoke()` 调用，模拟 `http://tauri.local` 端点
- 关键文件：`tests/msw/tauriMocks.ts`（mock 层）、`tests/msw/state.ts`（测试状态）、`tests/msw/handlers.ts`
- Setup 文件：`tests/setupGlobals.ts`（polyfill）、`tests/setupTests.ts`（MSW server 生命周期）
- 测试目录：`tests/components/`、`tests/hooks/`、`tests/config/`、`tests/utils/`、`tests/integration/`

### 后端

- Rust 测试在 `src-tauri/tests/`（集成测试）和各模块的 `#[cfg(test)]` 中
- `--features test-hooks` 启用测试专用 hooks
- 集成测试需要 `mkdir -p dist` 占位（CI 中也是这样做的）

## Conventions

- 代码注释和错误信息用中文（匹配 `AppError` 中的 `"配置错误"`, `"JSON 解析错误"` 等风格）
- Rust crate name 用下划线：`cc_switch_lib`
- Zod v4 做表单验证（`@hookform/resolvers` + `zod`）
- `serde_json` 启用 `preserve_order` feature（保持 JSON 字段顺序）
- shadcn/ui 组件（Radix UI 原语），组件在 `src/components/ui/`

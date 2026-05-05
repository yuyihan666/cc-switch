# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **本文件即 `AGENTS.md`**:`CLAUDE.md` 是 `AGENTS.md` 的软链(由 `setup-agent-links.sh` 维护),改 `AGENTS.md` 即同步生效。若软链断裂,执行 `bash setup-agent-links.sh` 修复。

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

- `commands/` 中每个域一个文件或子模块(30+ 文件),全部在 `mod.rs` 中 `pub use *` 导出
- `services/` 是业务逻辑层,主要入口:`ProviderService`, `McpService`, `PromptService`, `SkillService`, `ProxyService`, `ConfigService`(部分服务是子目录,如 `services/provider/`、`services/webdav_sync/`)
- `database/dao/` 每个域一个文件(现有 12 个 DAO),`Database` struct 通过 `impl` 块提供 DAO 方法
- `proxy/` 是独立的本地 HTTP 代理子系统(30+ 文件),支持多 Provider 故障转移和请求透传
- **`src-tauri/src/lib.rs`(1777 行)是 Tauri 启动器 + 命令注册中心**:`tauri::generate_handler![]` 块在第 1032 行,新增 Tauri command 必须在此注册,否则前端 `invoke()` 找不到

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

## PR Process

### 分支与提交

- 分支命名：`feat/xxx`、`fix/xxx`、`chore/xxx`
- 提交格式遵循 [Conventional Commits](https://www.conventionalcommits.org/)：`feat(provider): add support for new provider`

### 提交前 Checklist

改了 Rust 代码必须跑完这 6 步再 push，CI 不会替你格式化：

```bash
pnpm typecheck && pnpm format:check && pnpm test:unit
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

如果 `cargo fmt --check` 报 diff，直接跑 `cargo fmt --manifest-path src-tauri/Cargo.toml` 修复后再提交。

改了用户可见文本需要同步更新 3 个 locale 文件:`src/i18n/locales/{en,zh,ja}.json`

### 用 gh 提交 PR 的完整流程

以下流程适用于 AI agent 自主操作或人工通过 CLI 提交：

```bash
# 1. 从最新 main 创建分支
git checkout main && git pull
git checkout -b feat/my-feature

# 2. 编码 + 验证（见上方 Checklist）
# ... 改代码 ...
pnpm typecheck && pnpm format:check && pnpm test:unit
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

# 3. 提交
git add <具体文件>        # 不要 git add -A，避免提交无关文件
git commit                # 用 Conventional Commits 格式

# 4. 推送（首次推送用 -u）
git push -u origin feat/my-feature

# 5. 创建 PR
gh pr create --title "feat(scope): 简短标题" --body "$(cat <<'EOF'
## Summary
- 改动要点 1
- 改动要点 2

## Test plan
- [ ] 验证步骤 1
- [ ] 验证步骤 2

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

**注意事项**：
- `git add` 指定具体文件，不要用 `-A` 或 `.`，避免提交 `.env`、临时文件等
- PR 标题不超过 70 字符，body 包含 Summary + Test plan
- 推送前确认 `cargo fmt --check` 和 `cargo clippy` 都通过，否则 CI 必挂
- 如果是 fork 仓库的 PR，`gh pr create` 会自动处理 fork + remote

## AI Agent Pitfalls

以下是 AI agent 在本项目中容易犯的错误,改动前对照检查(只列**反直觉、正文未覆盖**的几条):

1. **改了数据库 schema 但没递增 `SCHEMA_VERSION`** — 改表结构必须同时改 `src-tauri/src/database/schema.rs`,递增版本号并添加迁移逻辑(当前 `SCHEMA_VERSION = 10`,定义在 `database/mod.rs`)
2. **从项目根目录直接跑 `cargo fmt`** — 必须加 `--manifest-path src-tauri/Cargo.toml`,否则找不到 crate
3. **新增 Tauri command 不用 camelCase** — Tauri 2.0 要求 command 名用 camelCase(如 `getProviders`),不是 snake_case;且必须在 `src-tauri/src/lib.rs:1032` 的 `tauri::generate_handler![]` 注册
4. **写配置文件没用原子写入** — 必须用 temp file + rename 模式,参考 `src-tauri/src/config.rs:204` 的 `atomic_write` 函数(不在 `services/`)
5. **CI Node 20 vs 本地 Node 22** — `.node-version` 是 `22.12.0`,但 `.github/workflows/ci.yml` 用 `node-version: "20"`。本地 typecheck/test 跑通不代表 CI 通过,push 后留意 CI 结果

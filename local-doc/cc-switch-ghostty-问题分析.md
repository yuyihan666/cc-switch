# cc-switch Ghostty 终端启动问题分析

## 问题现象

在 cc-switch 中设置 Ghostty 为首选终端，点击"打开提供商终端"时出现三个异常：

1. **`claude: command not found`** — 终端启动后报错，找不到 `claude` 命令
2. **打开了两个终端窗口** — 一次点击弹出了两个 Ghostty 窗口
3. **Ghostty 弹出确认对话框** — Ghostty 提示"是否允许外部应用执行命令"

实际终端输出：

```
Last login: Tue May  5 18:35:13 on ttys020
Using provider-specific claude config:
/var/folders/pm/.../claude_43a32424-483b-440f-80fa-9ce870836705_7188.json
/var/folders/pm/.../cc_switch_launcher_7188.sh: line 7: claude: command not found

The default interactive shell is now zsh.
To update your account to use zsh, please run `chsh -s /bin/zsh`.
bash-3.2$
```

---

## 排查过程

### Step 1：查看生成的启动脚本

cc-switch 在 `/tmp/` 下生成临时脚本，内容如下：

```bash
#!/bin/bash
trap 'rm -f "/tmp/claude_xxx.json" "/tmp/cc_switch_launcher_7188.sh"' EXIT
cd '/Users/yyhcoco/yyh/temp' || exit 1

echo "Using provider-specific claude config:"
echo "/tmp/claude_xxx.json"
claude --settings "/tmp/claude_xxx.json"
exec bash --norc --noprofile
```

脚本做了三件事：切换到项目目录 → 运行 `claude --settings` → claude 退出后 `exec` 到 bash 保持终端不关闭。

### Step 2：确定 Ghostty 如何被调用

源码位置：`src-tauri/src/commands/misc.rs:1066-1096`

```rust
fn launch_macos_open_app(app_name, script_file, use_e_flag) {
    Command::new("open")
        .arg("-a").arg("Ghostty")
        .arg("--args")
        .arg("-e")
        .arg("bash").arg(script_file);
}
```

实际执行的命令是：

```
open -a Ghostty --args -e bash /tmp/cc_switch_launcher_7188.sh
```

### Step 3：对比 session manager 的 Ghostty 启动方式

源码位置：`src-tauri/src/session_manager/terminal/mod.rs:82-113`

```rust
fn launch_ghostty(command, cwd) {
    let shell = std::env::var("SHELL").unwrap_or("/bin/zsh");

    Command::new("open")
        .arg("-na").arg("Ghostty")
        .arg("--args")
        .arg("--quit-after-last-window-closed=true")
        .arg("--working-directory=...")
        .arg("-e").arg(shell)
        .arg("-l")       // ← 登录 shell
        .arg("-c").arg(command);
}
```

实际命令类似：

```
open -na Ghostty --args --quit-after-last-window-closed=true -e /bin/zsh -l -c "claude --resume xxx"
```

**关键差异**：

| | provider terminal | session manager |
|---|---|---|
| 命令形式 | 脚本文件 | 内联命令 |
| Shell 类型 | `bash`（非登录） | `/bin/zsh -l`（登录 shell） |
| PATH | launchd 最小 PATH | 用户完整 PATH |
| open 参数 | `-a` | `-na` |

### Step 4：定位根因

**问题 1 — `claude: command not found`**

调用链路：

```
cc-switch (Rust) → open -a Ghostty --args -e bash script.sh
                  → Ghostty 从 launchd 继承 PATH
                  → PATH = /usr/bin:/bin:/usr/sbin:/sbin
                  → bash script.sh 找不到 claude（安装在 ~/.local/bin/claude）
```

`open -a` 启动的进程从 macOS 的 `launchd` 继承环境变量，而 launchd 的 PATH 只包含系统基本路径。脚本中的 `exec bash --norc --noprofile` 进一步禁止加载 `.bashrc`/`.bash_profile`，彻底丢失用户 PATH。

而 session manager 使用 `-e /bin/zsh -l`，`-l` 标志使 zsh 作为登录 shell 启动，自动 source `~/.zprofile`/`~/.zshrc`，PATH 完整。

**问题 2 — 打开两个终端**

推测原因：
- 第一个窗口：Ghostty 执行脚本，`claude` 报错后 `exec bash --norc --noprofile` 留下一个 bash shell
- 第二个窗口：Ghostty 的确认对话框可能伴随一个额外窗口，或用户在 bash 中手动再次执行脚本触发了新窗口

实际日志显示两个不同的 tty（`ttys020` 和 `ttys012`），确认确实打开了两个终端。

**问题 3 — Ghostty 确认对话框**

Ghostty 的安全机制。当外部应用通过 `open -a Ghostty --args -e ...` 触发命令执行时，Ghostty 会弹出确认提示。这是 Ghostty 自身行为，无法从调用方绕过。

---

## 修复方案

### 修改位置

`src-tauri/src/commands/misc.rs`

### 修改 1：Rust 侧解析 claude 绝对路径

在 `launch_terminal_with_env()` 函数（第 855 行）中，调用终端启动函数之前，通过 `which claude` 解析绝对路径：

```rust
let claude_path = std::process::Command::new("which")
    .arg("claude")
    .output()
    .ok()
    .and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    })
    .unwrap_or_else(|| "claude".to_string());
```

然后将 `claude_path` 传入 `launch_macos_terminal()` 和 `launch_linux_terminal()`。

### 修改 2：脚本使用绝对路径 + 用户登录 shell

`launch_macos_terminal()` 第 926-933 行，脚本模板改动：

```diff
- claude --settings "{config_path}"
+ {claude_path} --settings "{config_path}"

- exec bash --norc --noprofile
+ exec "$SHELL" -l
```

`launch_linux_terminal()` 同样修改。

### 效果

- `claude` 通过绝对路径调用，不再依赖运行时的 PATH
- claude 退出后进入用户的默认登录 shell（zsh），而不是非登录 bash
- 所有终端类型（Ghostty、iTerm2、Alacritty 等）都受益
- 与 session manager 的行为保持一致

### 关于 Ghostty 确认对话框

这是 Ghostty 自身的安全策略，无法从 cc-switch 代码绕过。如果用户感到困扰，可以在 Ghostty 设置中调整安全选项，或首次确认时勾选"记住选择"。

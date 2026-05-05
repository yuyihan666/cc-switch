#!/bin/bash
# setup-agent-links.sh
# 统一 AI 工具配置目录和文件到 .agents / AGENTS.md 作为唯一源

set -e

# ========== 配置区 ==========
# 唯一源目录
SOURCE_DIR=".agents"

# 需要映射到 SOURCE_DIR 的目录列表（按需增删）
LINK_DIRS=(".qoder" ".trae" ".claude")

# 唯一源文件
SOURCE_FILE="AGENTS.md"

# 需要映射到 SOURCE_FILE 的文件列表（按需增删）
LINK_FILES=("CLAUDE.md")
# ============================

echo "🔧 开始配置 AI 工具统一目录..."

# 1. 确保唯一源目录存在且是真实目录
if [ -L "$SOURCE_DIR" ]; then
  echo "⚠️  $SOURCE_DIR 是软连接，正在解除..."
  rm -f "$SOURCE_DIR"
  mkdir -p "$SOURCE_DIR"
elif [ ! -d "$SOURCE_DIR" ]; then
  echo "📁 创建 $SOURCE_DIR 目录"
  mkdir -p "$SOURCE_DIR"
fi

# 2. 迁移目录内容到唯一源
for dir in "${LINK_DIRS[@]}"; do
  # 已经是软连接 → 跳过
  if [ -L "$dir" ]; then
    echo "✅ $dir 已经是 $SOURCE_DIR 的软连接，跳过"
    continue
  fi
  
  # 是真实目录 → 迁移内容
  if [ -d "$dir" ]; then
    echo "📦 迁移 $dir 内容到 $SOURCE_DIR..."
    cp -rn "$dir"/. "$SOURCE_DIR"/ 2>/dev/null || true
    rm -rf "$dir"
    ln -sfn "$SOURCE_DIR" "$dir"
    echo "✅ $dir 已迁移并创建软连接"
    continue
  fi
  
  # 不存在 → 创建软连接
  if [ ! -e "$dir" ]; then
    ln -sfn "$SOURCE_DIR" "$dir"
    echo "✅ 创建 $dir 软连接"
  fi
done

# 3. 处理唯一源文件 / 映射文件
# 确保唯一源文件存在
if [ ! -f "$SOURCE_FILE" ]; then
  echo "📄 创建 $SOURCE_FILE"
  touch "$SOURCE_FILE"
fi

# 处理映射文件
for file in "${LINK_FILES[@]}"; do
  if [ -L "$file" ]; then
    echo "✅ $file 已经是 $SOURCE_FILE 的软连接，跳过"
  elif [ -f "$file" ]; then
    echo "📦 迁移 $file 内容到 $SOURCE_FILE..."
    # 如果唯一源文件为空，直接覆盖；否则追加
    if [ ! -s "$SOURCE_FILE" ]; then
      mv "$file" "$SOURCE_FILE"
    else
      cat "$file" >> "$SOURCE_FILE"
      rm -f "$file"
    fi
    ln -sfn "$SOURCE_FILE" "$file"
    echo "✅ $file 已迁移并创建软连接"
  else
    ln -sfn "$SOURCE_FILE" "$file"
    echo "✅ 创建 $file 软连接"
  fi
done

echo ""
echo "✅ 配置完成！"
echo ""
echo "📋 目录状态:"
ls -la "$SOURCE_DIR" "${LINK_DIRS[@]}" 2>/dev/null || true
echo ""
echo "📋 文件状态:"
ls -la "$SOURCE_FILE" "${LINK_FILES[@]}" 2>/dev/null || true

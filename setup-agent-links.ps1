#Requires -Version 5.1
<#
.SYNOPSIS
    统一 AI 工具配置目录和文件到 .agents / AGENTS.md 作为唯一源（Windows 版）

.DESCRIPTION
    将 .qoder, .trae, .claude 等目录通过 Junction 映射到 .agents，
    将 CLAUDE.md 通过 HardLink 映射到 AGENTS.md。
    不需要管理员权限。

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\setup-agent-links.ps1
#>

$ErrorActionPreference = "Stop"

# ========== 配置区 ==========
$SOURCE_DIR = ".agents"
$LINK_DIRS = @(".qoder", ".trae", ".claude")
$SOURCE_FILE = "AGENTS.md"
$LINK_FILES = @("CLAUDE.md")
# ============================

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
if (-not $scriptDir) { $scriptDir = "." }

function Test-CanCreateSymlinks {
    try {
        $devMode = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" -Name "AllowDevelopmentWithoutDevLicense" -ErrorAction SilentlyContinue
        if ($devMode.AllowDevelopmentWithoutDevLicense -eq 1) { return $true }
    } catch {}

    try {
        $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = New-Object Security.Principal.WindowsPrincipal($identity)
        return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    } catch {}

    return $false
}

function Test-IsJunction {
    param([string]$Path)
    $item = Get-Item $Path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return $false }
    return ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -and $item.PSIsContainer
}

function Test-IsHardLink {
    param([string]$Path)
    $item = Get-Item $Path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return $false }
    return $item.LinkType -eq "HardLink"
}

function Test-IsSymbolicLink {
    param([string]$Path)
    $item = Get-Item $Path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return $false }
    return $item.LinkType -eq "SymbolicLink"
}

function Get-JunctionTarget {
    param([string]$Path)
    $item = Get-Item $Path -Force
    return $item.Target
}

$canCreateSymlinks = Test-CanCreateSymlinks

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  AI 工具配置统一脚本 (Windows)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ---- Phase 1: 确保 SOURCE_DIR 是真实目录 ----
$sourceDirPath = Join-Path $scriptDir $SOURCE_DIR

if (Test-IsJunction $sourceDirPath) {
    $target = Get-JunctionTarget $sourceDirPath
    Write-Host "[!] $SOURCE_DIR 是 Junction (指向 $target)，正在解除..." -ForegroundColor Yellow
    Remove-Item $sourceDirPath -Force
    New-Item -ItemType Directory -Path $sourceDirPath -Force | Out-Null
    Write-Host "[OK] $SOURCE_DIR 已重建为真实目录" -ForegroundColor Green
} elseif (Test-Path $sourceDirPath -PathType Leaf) {
    # core.symlinks=false 导致的普通文件
    Write-Host "[!] $SOURCE_DIR 是普通文件（非预期），正在删除..." -ForegroundColor Yellow
    Remove-Item $sourceDirPath -Force
    New-Item -ItemType Directory -Path $sourceDirPath -Force | Out-Null
    Write-Host "[OK] $SOURCE_DIR 已创建为目录" -ForegroundColor Green
} elseif (-not (Test-Path $sourceDirPath)) {
    New-Item -ItemType Directory -Path $sourceDirPath -Force | Out-Null
    Write-Host "[OK] 创建 $SOURCE_DIR 目录" -ForegroundColor Green
} else {
    Write-Host "[OK] $SOURCE_DIR 已是真实目录" -ForegroundColor Green
}

# ---- Phase 2: 处理目录映射 ----
foreach ($dir in $LINK_DIRS) {
    $dirPath = Join-Path $scriptDir $dir

    if (Test-IsJunction $dirPath) {
        $target = (Get-JunctionTarget $dirPath).TrimEnd('\')
        $expectedTarget = (Resolve-Path $sourceDirPath -ErrorAction SilentlyContinue).Path.TrimEnd('\')
        if ($target -eq $expectedTarget) {
            Write-Host "[OK] $dir 已是 $SOURCE_DIR 的 Junction，跳过" -ForegroundColor Green
            continue
        } else {
            Write-Host "[!] $dir 是 Junction 但指向 $target，正在修正..." -ForegroundColor Yellow
            Remove-Item $dirPath -Force
        }
    }

    if (Test-Path $dirPath -PathType Container) {
        # 真实目录 → 迁移内容
        Write-Host "[..] 迁移 $dir 内容到 $SOURCE_DIR..." -ForegroundColor Cyan
        robocopy $dirPath $sourceDirPath /E /XC /XN /XO /NFL /NDL /NJH /NJS /NC /NS | Out-Null
        # robocopy 返回 0-7 都算成功
        if ($LASTEXITCODE -gt 7) {
            Write-Host "[!!] robocopy 失败 (退出码 $LASTEXITCODE)" -ForegroundColor Red
        }
        Remove-Item $dirPath -Recurse -Force
        New-Item -ItemType Junction -Path $dirPath -Target $sourceDirPath | Out-Null
        Write-Host "[OK] $dir 已迁移并创建 Junction" -ForegroundColor Green
    } elseif (Test-Path $dirPath -PathType Leaf) {
        # git core.symlinks=false 导致的普通文件（内容是 ".agents\n"）
        Write-Host "[!] $dir 是普通文件（git 未创建链接），正在替换为 Junction..." -ForegroundColor Yellow
        Remove-Item $dirPath -Force
        New-Item -ItemType Junction -Path $dirPath -Target $sourceDirPath | Out-Null
        Write-Host "[OK] $dir 已创建 Junction" -ForegroundColor Green
    } else {
        New-Item -ItemType Junction -Path $dirPath -Target $sourceDirPath | Out-Null
        Write-Host "[OK] 创建 $dir Junction" -ForegroundColor Green
    }
}

# ---- Phase 3: 确保 SOURCE_FILE 存在 ----
$sourceFilePath = Join-Path $scriptDir $SOURCE_FILE

if (-not (Test-Path $sourceFilePath)) {
    New-Item -ItemType File -Path $sourceFilePath -Force | Out-Null
    Write-Host "[OK] 创建 $SOURCE_FILE" -ForegroundColor Green
}

# ---- Phase 4: 处理文件映射 ----
foreach ($file in $LINK_FILES) {
    $filePath = Join-Path $scriptDir $file

    # 检查是否已经是硬链接或符号链接指向同一文件
    if (Test-IsSymbolicLink $filePath) {
        Write-Host "[OK] $file 已经是符号链接，跳过" -ForegroundColor Green
        continue
    }

    if (Test-IsHardLink $filePath) {
        Write-Host "[OK] $file 已经是硬链接，跳过" -ForegroundColor Green
        continue
    }

    if (Test-Path $filePath -PathType Leaf) {
        # 真实文件 → 合并内容
        $sourceContent = Get-Content $sourceFilePath -Raw -ErrorAction SilentlyContinue
        $fileContent = Get-Content $filePath -Raw -ErrorAction SilentlyContinue

        if ([string]::IsNullOrEmpty($sourceContent)) {
            # 源文件为空 → 直接移动
            Move-Item $filePath $sourceFilePath -Force
            Write-Host "[OK] $file 内容已移至 $SOURCE_FILE" -ForegroundColor Green
        } else {
            # 源文件有内容 → 追加
            Add-Content -Path $sourceFilePath -Value $fileContent
            Remove-Item $filePath -Force
            Write-Host "[OK] $file 内容已追加到 $SOURCE_FILE" -ForegroundColor Green
        }
    } elseif (Test-Path $filePath -PathType Container) {
        # 不太可能，但防御性处理
        Write-Host "[!!] $file 是目录而非文件，跳过" -ForegroundColor Red
        continue
    }

    # 创建链接
    if ($canCreateSymlinks) {
        try {
            New-Item -ItemType SymbolicLink -Path $filePath -Target $sourceFilePath | Out-Null
            Write-Host "[OK] 创建 $file 符号链接 -> $SOURCE_FILE" -ForegroundColor Green
        } catch {
            # 回退到硬链接
            New-Item -ItemType HardLink -Path $filePath -Target $sourceFilePath | Out-Null
            Write-Host "[OK] 创建 $file 硬链接 <-> $SOURCE_FILE" -ForegroundColor Green
        }
    } else {
        New-Item -ItemType HardLink -Path $filePath -Target $sourceFilePath | Out-Null
        Write-Host "[OK] 创建 $file 硬链接 <-> $SOURCE_FILE" -ForegroundColor Green
    }
}

# ---- Phase 5: 状态报告 ----
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  配置状态" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

Write-Host ""
Write-Host "  目录:" -ForegroundColor Yellow
$allDirs = @($SOURCE_DIR) + $LINK_DIRS
foreach ($dir in $allDirs) {
    $dirPath = Join-Path $scriptDir $dir
    if (Test-IsJunction $dirPath) {
        $target = Get-JunctionTarget $dirPath
        Write-Host ("  [Junction]  {0} -> {1}" -f $dir, $target) -ForegroundColor Green
    } elseif (Test-Path $dirPath -PathType Container) {
        Write-Host "  [Directory] $dir" -ForegroundColor White
    } elseif (Test-Path $dirPath -PathType Leaf) {
        Write-Host "  [File]      $dir (异常)" -ForegroundColor Red
    } else {
        Write-Host "  [Missing]   $dir" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "  文件:" -ForegroundColor Yellow
$allFiles = @($SOURCE_FILE) + $LINK_FILES
foreach ($file in $allFiles) {
    $filePath = Join-Path $scriptDir $file
    if (Test-IsSymbolicLink $filePath) {
        $target = (Get-Item $filePath -Force).Target
        Write-Host ("  [SymLink]   {0} -> {1}" -f $file, $target) -ForegroundColor Green
    } elseif (Test-IsHardLink $filePath) {
        Write-Host ("  [HardLink]  {0} <-> {1}" -f $file, $SOURCE_FILE) -ForegroundColor Green
    } elseif (Test-Path $filePath -PathType Leaf) {
        $size = (Get-Item $filePath).Length
        Write-Host ("  [File]      {0} ({1} bytes)" -f $file, $size) -ForegroundColor White
    } else {
        Write-Host "  [Missing]   $file" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  配置完成！" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""

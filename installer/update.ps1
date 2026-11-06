param([Parameter(Mandatory=$true)][string]$WorkPath, [Parameter(Mandatory=$true)][string]$TargetExe, [int]$ParentProcessId, [ValidateSet('zh-CN','en')][string]$Language = 'zh-CN')
$ErrorActionPreference = 'Stop'
function Text([string]$Chinese, [string]$English) { if ($Language -eq 'en') { $English } else { $Chinese } }
# 更新只使用当前 PowerShell 自带模块，避免继承其他终端版本的模块路径。
$env:PSModulePath = [IO.Path]::Combine($PSHOME, 'Modules')
$cacheRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'islo\Updates')).TrimEnd('\')
$workRoot = [IO.Path]::GetFullPath($WorkPath).TrimEnd('\')
$target = [IO.Path]::GetFullPath($TargetExe)
$targetDir = Split-Path $target -Parent
$staged = Join-Path $targetDir ('islo-update-' + [Guid]::NewGuid().ToString('N') + '.exe')
$backup = Join-Path $workRoot 'previous.exe'
$cliTarget = Join-Path $targetDir 'islo-cli.exe'
$cliStaged = Join-Path $targetDir ('islo-cli-update-' + [Guid]::NewGuid().ToString('N') + '.exe')
$cliBackup = Join-Path $workRoot 'previous-cli.exe'
$hasCli = $false
$cliExisted = $false
$cliReplaced = $false
$replaced = $false
$exited = $false
function Assert-PlainPath([string]$PathToCheck) {
    $ancestor = [IO.Path]::GetFullPath($PathToCheck)
    while ($ancestor) {
        if ((Test-Path -LiteralPath $ancestor) -and ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw (Text '更新目录不能包含目录链接。' 'Update folders cannot contain directory links.') }
        $ancestor = Split-Path $ancestor -Parent
    }
}
try {
    if (-not $workRoot.StartsWith($cacheRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw (Text '更新缓存目录无效。' 'Invalid update cache folder.') }
    Assert-PlainPath $workRoot
    Assert-PlainPath $target
    if (-not (Test-Path -LiteralPath $target -PathType Leaf)) { throw (Text '找不到当前程序。' 'Current executable not found.') }
    $version = (Get-Content -LiteralPath (Join-Path $workRoot 'version.txt') -Raw).Trim()
    if ($version -notmatch '^\d+\.\d+\.\d+(?:\.\d+)?$') { throw (Text '更新版本号无效。' 'Invalid update version.') }
    $archive = Join-Path $workRoot 'update.zip'
    $expectedHash = (Get-Content -LiteralPath (Join-Path $workRoot 'hash.txt') -Raw).Trim()
    if ($expectedHash -notmatch '^[a-fA-F0-9]{64}$' -or (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expectedHash) { throw (Text '更新包校验失败。' 'Update package verification failed.') }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $name = 'islo-v' + $version + '-windows-x64/islo.exe'
        $entries = @($zip.Entries | Where-Object {$_.FullName.Replace('\','/') -ceq $name})
        if ($entries.Count -ne 1 -or $entries[0].Length -gt 200MB) { throw (Text '更新包中缺少唯一的islo程序。' 'Update package must contain exactly one islo executable.') }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entries[0], $staged)
        $cliName = 'islo-v' + $version + '-windows-x64/islo-cli.exe'
        $cliEntries = @($zip.Entries | Where-Object {$_.FullName.Replace('\','/') -ceq $cliName})
        if ($cliEntries.Count -gt 1 -or ($cliEntries.Count -eq 1 -and $cliEntries[0].Length -gt 200MB)) { throw (Text '更新包中的 CLI 无效。' 'Invalid CLI in update package.') }
        $hasCli = $cliEntries.Count -eq 1
        if ($hasCli) { [IO.Compression.ZipFileExtensions]::ExtractToFile($cliEntries[0], $cliStaged) }
        elseif (Test-Path -LiteralPath $cliTarget) { throw (Text '更新包缺少已有的 CLI，不能只更新桌面程序。' 'Update package is missing the installed CLI. Both executables must be updated together.') }
    } finally { $zip.Dispose() }
    $actual = [Diagnostics.FileVersionInfo]::GetVersionInfo($staged).FileVersion
    if ($actual -ne $version -and $actual -ne "$version.0") { throw (Text '安装包版本不匹配。' 'Package version mismatch.') }
    if ($hasCli) {
        $cliActual = [Diagnostics.FileVersionInfo]::GetVersionInfo($cliStaged).FileVersion
        if ($cliActual -ne $version -and $cliActual -ne "$version.0") { throw (Text 'CLI 版本不匹配。' 'CLI version mismatch.') }
        Assert-PlainPath $cliTarget
    }
    Set-Content -LiteralPath (Join-Path $workRoot 'ready') -Value 'ok'
    if ($ParentProcessId -gt 0) {
        $parent = Get-Process -Id $ParentProcessId -ErrorAction SilentlyContinue
        if ($parent -and -not $parent.WaitForExit(60000)) { throw (Text 'islo尚未退出，请稍后重试。' 'islo has not exited yet. Try again later.') }
    }
    $exited = $true
    # 缓存可能启用 EFS；先复制备份，避免原子替换时跨加密边界移动旧程序。
    [IO.File]::Copy($target, $backup, $true)
    [IO.File]::Replace($staged, $target, [NullString]::Value)
    $replaced = $true
    if ($hasCli) {
        $cliExisted = Test-Path -LiteralPath $cliTarget
        if ($cliExisted) {
            [IO.File]::Copy($cliTarget, $cliBackup, $true)
            [IO.File]::Replace($cliStaged, $cliTarget, [NullString]::Value)
        } else { [IO.File]::Move($cliStaged, $cliTarget) }
        $cliReplaced = $true
    }
    Start-Process -FilePath $target -WorkingDirectory $targetDir -WindowStyle Hidden
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\islo'
    $registered = Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
    if ($registered -and [string]::Equals($registered.InstallLocation.TrimEnd('\'), $targetDir, [StringComparison]::OrdinalIgnoreCase)) {
        Set-ItemProperty -LiteralPath $key -Name DisplayVersion -Value $version -ErrorAction SilentlyContinue
    }
    # 只清理已经确认位于专用缓存内的本次更新目录。
    if ($workRoot.StartsWith($cacheRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
        try { Assert-PlainPath $workRoot; Remove-Item -LiteralPath $workRoot -Recurse -Force } catch { }
    }
} catch {
    $message = $_.Exception.Message
    if ($replaced -and (Test-Path -LiteralPath $backup)) { [IO.File]::Copy($backup, $target, $true) }
    if ($cliReplaced) {
        if ($cliExisted) { [IO.File]::Copy($cliBackup, $cliTarget, $true) }
        elseif (Test-Path -LiteralPath $cliTarget) { Remove-Item -LiteralPath $cliTarget -Force }
    }
    if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged -Force }
    if (Test-Path -LiteralPath $cliStaged) { Remove-Item -LiteralPath $cliStaged -Force }
    if (-not $exited) {
        if ($workRoot.StartsWith($cacheRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
            [IO.File]::WriteAllText((Join-Path $workRoot 'error.txt'), $message)
        }
        exit 1
    }
    if ($exited) { Start-Process -FilePath $target -WorkingDirectory $targetDir -WindowStyle Hidden }
    Add-Type -AssemblyName System.Windows.Forms
    $failureText = Text "更新未完成：$message`r`n原版本已保留，可稍后重试或通过浏览器下载。" "Update failed: $message`r`nThe previous version was kept. Retry later or download in your browser."
    [System.Windows.Forms.MessageBox]::Show($failureText, (Text 'islo更新' 'islo update')) | Out-Null
    exit 1
}

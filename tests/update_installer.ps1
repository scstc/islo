# 使用独立目录和最小测试程序验证更新，不启动或替换日常使用的islo。
param([switch]$EncryptedCache, [string]$FixtureVersion = '9.8.7', [ValidateSet('zh-CN','en')][string]$Language = 'zh-CN')
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$testRoot = Join-Path $project ('target\update-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$oldLocal = $env:LOCALAPPDATA
try {
    $env:LOCALAPPDATA = if ($EncryptedCache) { Join-Path $oldLocal ('isloUpdateTest-' + [Guid]::NewGuid().ToString('N')) } else { Join-Path $testRoot 'local' }
    New-Item -ItemType Directory -Path $env:LOCALAPPDATA -Force | Out-Null
    if ($EncryptedCache -and -not ((Get-Item -LiteralPath $env:LOCALAPPDATA).Attributes -band [IO.FileAttributes]::Encrypted)) { throw '测试缓存目录必须启用 EFS 加密' }
    $compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    $source = Join-Path $testRoot 'fixture.cs'
    '[assembly:System.Reflection.AssemblyFileVersion("FIXTURE_VERSION")] class Fixture { static void Main() { System.IO.File.WriteAllText(System.IO.Path.Combine(System.AppDomain.CurrentDomain.BaseDirectory,"started.txt"),"ok"); } }'.Replace('FIXTURE_VERSION',$FixtureVersion) | Set-Content -LiteralPath $source
    $fixture = Join-Path $testRoot 'fixture.exe'
    & $compiler /nologo /target:winexe "/out:$fixture" $source
    if ($LASTEXITCODE -ne 0) { throw '测试程序编译失败' }
    $badCliSource = Join-Path $testRoot 'bad-cli.cs'
    [IO.File]::ReadAllText($source).Replace($FixtureVersion,'1.2.3.4') | Set-Content -LiteralPath $badCliSource
    $badCliFixture = Join-Path $testRoot 'bad-cli.exe'
    & $compiler /nologo /target:winexe "/out:$badCliFixture" $badCliSource
    if ($LASTEXITCODE -ne 0) { throw '错误版本 CLI 测试程序编译失败' }
    $installer = Join-Path $testRoot 'apply.ps1'
    [IO.File]::WriteAllText($installer, [IO.File]::ReadAllText((Join-Path $project 'installer\update.ps1')), [Text.UTF8Encoding]::new($true))
    $targetDir = Join-Path $testRoot 'installed'
    New-Item -ItemType Directory -Path $targetDir | Out-Null
    $target = Join-Path $targetDir 'islo.exe'
    $data = Join-Path $targetDir 'settings.json'
    Set-Content -LiteralPath $data -Value 'keep-user-data'
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    foreach ($case in @('success','inherited-shell','legacy-no-cli','bad-hash','wrong-version','cli-wrong-version','missing-cli','traversal')) {
        Remove-Item -LiteralPath (Join-Path $targetDir 'started.txt') -ErrorAction SilentlyContinue
        Set-Content -LiteralPath $target -Value 'previous-program'
        $before = (Get-FileHash -LiteralPath $target).Hash
        $cliTarget = Join-Path $targetDir 'islo-cli.exe'
        if ($case -eq 'legacy-no-cli') { if (Test-Path -LiteralPath $cliTarget) { Remove-Item -LiteralPath $cliTarget } }
        else { Set-Content -LiteralPath $cliTarget -Value 'previous-cli' }
        $cliBefore = if (Test-Path -LiteralPath $cliTarget) { (Get-FileHash -LiteralPath $cliTarget).Hash } else { '' }
        $work = Join-Path $env:LOCALAPPDATA ('islo\Updates\' + $case)
        New-Item -ItemType Directory -Path $work -Force | Out-Null
        $archive = Join-Path $work 'update.zip'
        $zip = [IO.Compression.ZipFile]::Open($archive, [IO.Compression.ZipArchiveMode]::Create)
        $versionParts = $FixtureVersion.Split('.')
        $versionParts[-1] = ([int]$versionParts[-1] + 1).ToString()
        $version = if ($case -eq 'wrong-version') {$versionParts -join '.'} else {$FixtureVersion}
        $entry = if ($case -eq 'traversal') {'../islo.exe'} else {"islo-v$version-windows-x64/islo.exe"}
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $fixture, $entry) | Out-Null
        if ($case -notin @('legacy-no-cli','missing-cli')) {
            $cliSource = if ($case -eq 'cli-wrong-version') {$badCliFixture} else {$fixture}
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $cliSource, "islo-v$version-windows-x64/islo-cli.exe") | Out-Null
        }
        $zip.Dispose()
        Set-Content -LiteralPath (Join-Path $work 'version.txt') -Value $version
        $hash = if ($case -eq 'bad-hash') {'a' * 64} else {(Get-FileHash -LiteralPath $archive).Hash}
        Set-Content -LiteralPath (Join-Path $work 'hash.txt') -Value $hash
        $shell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
        if ($case -eq 'inherited-shell') {
            # 模拟应用继承 PowerShell 7 的模块路径，绕过交互式 shell 对子进程环境的自动修正。
            $start = [Diagnostics.ProcessStartInfo]::new($shell)
            $start.UseShellExecute = $false
            $start.CreateNoWindow = $true
            $start.EnvironmentVariables['PSModulePath'] = $env:PSModulePath
            $start.Arguments = "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$installer`" -WorkPath `"$work`" -TargetExe `"$target`" -ParentProcessId 0 -Language $Language"
            $process = [Diagnostics.Process]::Start($start)
            if (-not $process.WaitForExit(30000)) { throw '继承终端环境的更新测试超时' }
            $code = $process.ExitCode
            $process.Dispose()
        } else {
            & $shell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $installer -WorkPath $work -TargetExe $target -ParentProcessId 0 -Language $Language
            $code = $LASTEXITCODE
        }
        if ($case -in @('success','inherited-shell','legacy-no-cli')) {
            if ($code -ne 0) { throw "更新安装失败：$case" }
            if ((Get-FileHash -LiteralPath $target).Hash -ne (Get-FileHash -LiteralPath $fixture).Hash) { throw '未替换为新版程序' }
            if ($case -eq 'legacy-no-cli') { if (Test-Path -LiteralPath $cliTarget) { throw '旧包不应产生 CLI' } }
            elseif ((Get-FileHash -LiteralPath $cliTarget).Hash -ne (Get-FileHash -LiteralPath $fixture).Hash) { throw 'CLI 未同步更新' }
            for ($attempt=0; $attempt -lt 50 -and -not (Test-Path -LiteralPath (Join-Path $targetDir 'started.txt')); $attempt++) { Start-Sleep -Milliseconds 100 }
            if (-not (Test-Path -LiteralPath (Join-Path $targetDir 'started.txt'))) { throw '更新后没有重启' }
        } else {
            if ($code -eq 0 -or -not (Test-Path -LiteralPath (Join-Path $work 'error.txt'))) { throw "未拒绝无效更新：$case" }
            $errorText = [IO.File]::ReadAllText((Join-Path $work 'error.txt'))
            if ($Language -eq 'en' -and $errorText -match '[\u4e00-\u9fff]') { throw "英文更新错误仍含中文：$case" }
            if ((Get-FileHash -LiteralPath $target).Hash -ne $before) { throw '无效更新修改了原程序' }
            if ((Get-FileHash -LiteralPath $cliTarget).Hash -ne $cliBefore) { throw '无效更新修改了 CLI' }
        }
        if ((Get-Content -LiteralPath $data -Raw).Trim() -ne 'keep-user-data') { throw '用户数据被修改' }
        Write-Output "通过：$case"
    }
} finally {
    $env:LOCALAPPDATA = $oldLocal
}

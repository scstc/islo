param([switch]$SkipBuild, [string]$Executable, [string]$CliExecutable, [string]$NsisCompiler, [string]$OutputDirectory, [string]$Toolchain = 'stable-x86_64-pc-windows-msvc')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    $version = (Get-Content -LiteralPath VERSION -Raw).Trim()
    if ($version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'VERSION must contain four numeric components' }
    $cargoHome = if ($env:CARGO_HOME) { [IO.Path]::GetFullPath($env:CARGO_HOME) } else { Join-Path $env:USERPROFILE '.cargo' }
    $targetDirectory = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $projectRoot 'target' }
    $privatePaths = @($targetDirectory, $projectRoot, $cargoHome, $env:USERPROFILE) | Where-Object { $_ } | Select-Object -Unique
    if (-not $SkipBuild) {
        $originalFlags = $env:CARGO_ENCODED_RUSTFLAGS
        $originalCL = $env:CL
        $originalJobs = $env:CARGO_BUILD_JOBS
        try {
            $env:CARGO_BUILD_JOBS = '1'
            $env:CARGO_ENCODED_RUSTFLAGS = @('-C', 'target-feature=+crt-static', "--remap-path-prefix=$cargoHome=cargo", "--remap-path-prefix=$projectRoot=islo", '-C', 'link-arg=/PDBALTPATH:%_PDB%') -join [char]0x1f
            if ($env:CARGO_TARGET_DIR) { $env:CARGO_ENCODED_RUSTFLAGS += [string][char]0x1f + "--remap-path-prefix=$targetDirectory=target" }
            # QuickJS 的 C 断言使用宽字符源文件名，MSVC 单独裁剪路径。
            $trimFlags = $privatePaths | ForEach-Object { '"/d1trimfile:' + $_ + '"' }
            $env:CL = (@($originalCL) + $trimFlags) -join ' '
            cargo "+$Toolchain" build --locked --release --bin islo --bin islo-cli --target x86_64-pc-windows-msvc
            if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
        } finally {
            $env:CL = $originalCL
            $env:CARGO_ENCODED_RUSTFLAGS = $originalFlags
            $env:CARGO_BUILD_JOBS = $originalJobs
        }
    }
    if (-not $Executable) {
        $Executable = Join-Path $targetDirectory 'x86_64-pc-windows-msvc\release\islo.exe'
    }
    $binary = Get-Item -LiteralPath $Executable
    if (-not $CliExecutable) { $CliExecutable = Join-Path $binary.DirectoryName 'islo-cli.exe' }
    $cliBinary = Get-Item -LiteralPath $CliExecutable
    & (Join-Path $PSScriptRoot 'check-runtime.ps1') -Executable $binary.FullName
    & (Join-Path $PSScriptRoot 'check-runtime.ps1') -Executable $cliBinary.FullName
    if ($binary.VersionInfo.FileVersion -ne $version) { throw 'Executable version mismatch' }
    if ($cliBinary.VersionInfo.FileVersion -ne $version) { throw 'CLI version mismatch' }
    foreach ($file in @($binary, $cliBinary)) {
        $bytes = [IO.File]::ReadAllBytes($file.FullName)
        foreach ($encoding in @([Text.Encoding]::UTF8, [Text.Encoding]::Unicode)) {
            $content = $encoding.GetString($bytes).Replace('/', '\')
            foreach ($private in $privatePaths) {
                if ($content.IndexOf($private.Replace('/', '\'), [StringComparison]::OrdinalIgnoreCase) -ge 0) { throw "发布程序 $($file.Name) 包含本机构建路径（$($encoding.WebName)）" }
            }
        }
    }
    if (-not $NsisCompiler) {
        $compiler = Get-Command makensis.exe -ErrorAction SilentlyContinue
        if ($compiler) { $NsisCompiler = $compiler.Source }
        elseif ($env:NSIS_HOME) { $NsisCompiler = Join-Path $env:NSIS_HOME 'makensis.exe' }
        else { $NsisCompiler = Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe' }
    }
    if (-not (Test-Path -LiteralPath $NsisCompiler)) { throw 'NSIS compiler not found; install NSIS or pass -NsisCompiler' }
    $packageName = "islo-v$version-windows-x64"
    if (-not $OutputDirectory) { $OutputDirectory = Join-Path $projectRoot 'dist' }
    $outputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
    $stagingDirectory = Join-Path $outputDirectory $packageName
    if (Test-Path -LiteralPath $stagingDirectory) { throw 'Package directory already exists' }
    New-Item -ItemType Directory -Path $stagingDirectory -Force | Out-Null
    Copy-Item -LiteralPath $binary.FullName -Destination (Join-Path $stagingDirectory 'islo.exe')
    Copy-Item -LiteralPath $cliBinary.FullName -Destination (Join-Path $stagingDirectory 'islo-cli.exe')
    # ZIP 保留给免安装使用和应用内更新；普通用户只需下载安装 EXE。
    @('LICENSE','LICENSES.md','THIRD_PARTY_NOTICES.txt') | ForEach-Object {
        Copy-Item -LiteralPath (Join-Path $projectRoot $_) -Destination $stagingDirectory -Recurse
    }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'installer\收集islo诊断.cmd') -Destination $stagingDirectory
    $setup = Join-Path $outputDirectory "islo-Setup-v$version-x64.exe"
    & $NsisCompiler /V2 /WX "/DAPP_VERSION=$version" "/DAPP_FILE_VERSION=$version" "/DPROJECT_DIR=$projectRoot" "/DSOURCE_EXE=$($binary.FullName)" "/DSOURCE_CLI=$($cliBinary.FullName)" "/DOUTPUT_FILE=$setup" (Join-Path $projectRoot 'installer\setup.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'Installer build failed' }
    $archive = Join-Path $outputDirectory "$packageName.zip"
    Compress-Archive -LiteralPath $stagingDirectory -DestinationPath $archive
    @($setup, $archive) | ForEach-Object {
        $checksum = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
        "$checksum  $([IO.Path]::GetFileName($_))"
    } | Set-Content -LiteralPath (Join-Path $outputDirectory 'SHA256SUMS.txt') -Encoding ascii
    Write-Output $setup
    Write-Output $archive
} finally { Pop-Location }

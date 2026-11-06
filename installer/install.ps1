param([string]$SourceExecutable)
if(-not $SourceExecutable){
    $packaged=Join-Path $PSScriptRoot '..\islo.exe'
    $SourceExecutable=if(Test-Path -LiteralPath $packaged){$packaged}else{Join-Path $PSScriptRoot '..\target\release\islo.exe'}
}
$ErrorActionPreference='Stop'
$sourcePath=[System.IO.Path]::GetFullPath($SourceExecutable)
if(-not(Test-Path -LiteralPath $sourcePath)){throw '找不到islo程序，请先完成构建。'}
$cliSource=Join-Path (Split-Path $sourcePath -Parent) 'islo-cli.exe'
if(Test-Path -LiteralPath $cliSource){
    if((Get-Item -LiteralPath $cliSource).VersionInfo.FileVersion -ne (Get-Item -LiteralPath $sourcePath).VersionInfo.FileVersion){throw '桌面程序与CLI版本不一致。'}
}
$installDir=Join-Path $env:LOCALAPPDATA 'Programs\islo'
$exePath=Join-Path $installDir 'islo.exe'
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Get-Process islo -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $exePath} | ForEach-Object {
    $null=$_.CloseMainWindow()
    if(-not $_.WaitForExit(3000)){Stop-Process -Id $_.Id; $_.WaitForExit()}
}
if(Test-Path -LiteralPath $exePath){Copy-Item -LiteralPath $exePath -Destination (Join-Path $installDir 'islo-previous.exe') -Force}
Copy-Item -LiteralPath $sourcePath -Destination $exePath -Force
if(Test-Path -LiteralPath $cliSource){Copy-Item -LiteralPath $cliSource -Destination (Join-Path $installDir 'islo-cli.exe') -Force}
if(Test-Path -LiteralPath (Join-Path $PSScriptRoot '..\THIRD_PARTY_NOTICES.txt')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot '..\THIRD_PARTY_NOTICES.txt') -Destination (Join-Path $installDir 'THIRD_PARTY_NOTICES.txt') -Force}
if(Test-Path -LiteralPath (Join-Path $PSScriptRoot '..\LICENSE')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot '..\LICENSE') -Destination (Join-Path $installDir 'LICENSE') -Force}
if(Test-Path -LiteralPath (Join-Path $PSScriptRoot '..\LICENSES.md')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot '..\LICENSES.md') -Destination (Join-Path $installDir 'LICENSES.md') -Force}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'uninstall.ps1') -Destination (Join-Path $installDir 'uninstall.ps1') -Force
$shell=New-Object -ComObject WScript.Shell
$entries=@(
    @((Join-Path $shell.SpecialFolders.Item('Desktop') 'islo.lnk'),''),
    @((Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\islo.lnk'),''),
    @((Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\islo设置.lnk'),'--settings')
)
foreach($entry in $entries){
    $link=$shell.CreateShortcut($entry[0]);$link.TargetPath=$exePath;$link.Arguments=$entry[1]
    $link.WorkingDirectory=$installDir;$link.IconLocation=$exePath+',0';$link.Description='islo';$link.Save()
}
$key='HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\islo'
New-Item -Path $key -Force | Out-Null
$version=(Get-Item -LiteralPath $exePath).VersionInfo.FileVersion
$psExe=Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
$values=@{DisplayName='islo';DisplayVersion=$version;Publisher='islo';InstallLocation=$installDir;DisplayIcon=$exePath+',0';UninstallString=('"'+$psExe+'" -NoProfile -ExecutionPolicy Bypass -File "'+(Join-Path $installDir 'uninstall.ps1')+'"')}
foreach($name in $values.Keys){New-ItemProperty -Path $key -Name $name -Value $values[$name] -PropertyType String -Force | Out-Null}
foreach($name in @('NoModify','NoRepair')){New-ItemProperty -Path $key -Name $name -Value 1 -PropertyType DWord -Force | Out-Null}
Start-Process -FilePath $exePath -WorkingDirectory $installDir -WindowStyle Hidden
Write-Output ('islo '+$version+' 已安装：'+$exePath)

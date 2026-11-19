param([ValidateSet('Release','Debug')][string]$Configuration='Release')
$ErrorActionPreference='Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (!(Test-Path -LiteralPath $vswhere)) { throw 'Install Visual Studio with Desktop development with C++.' }
$msbuild = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'MSBuild\**\Bin\MSBuild.exe' | Select-Object -First 1
if (!$msbuild) { throw 'The Visual C++ build tools were not found.' }
& $msbuild (Join-Path $PSScriptRoot 'Yami TB\Yami TB.vcxproj') /nologo /m /verbosity:minimal "/p:Configuration=$Configuration" /p:Platform=x64
if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
$artifactDirectory = Join-Path $PSScriptRoot 'artifacts'
New-Item -ItemType Directory -Path $artifactDirectory -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "Yami TB\x64\$Configuration\Yami.exe") -Destination (Join-Path $artifactDirectory 'Yami.exe') -Force
Write-Output (Join-Path $artifactDirectory 'Yami.exe')

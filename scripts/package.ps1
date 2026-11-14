<#
.SYNOPSIS
    Builds WebCa as an MSIX layout and either registers it for development or packs it.
.PARAMETER Mode
    Register: registers the loose layout for the current user (requires Developer Mode).
    Pack:     produces target\package\WebCa.msix with makeappx (requires the Windows SDK); sign it afterwards.
.PARAMETER Publisher
    Subject of the signing certificate; it must match the certificate used to sign the package.
#>
param(
    [ValidateSet('Register', 'Pack')] [string] $Mode = 'Register',
    [string] $Publisher = 'CN=WebCa',
    [ValidateSet('x64', 'arm64')] [string] $Arch = 'x64'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$target = @{ x64 = 'x86_64-pc-windows-msvc'; arm64 = 'aarch64-pc-windows-msvc' }[$Arch]

Push-Location $root
try {
    cargo build --release --locked -p webca --target $target
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object name -eq 'webca').version
    $layout = Join-Path $root 'target\package\layout'
    if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
    New-Item -ItemType Directory -Force (Join-Path $layout 'Assets') | Out-Null

    Copy-Item (Join-Path $root "target\$target\release\webca.exe") $layout
    $manifest = (Get-Content (Join-Path $root 'packaging\AppxManifest.xml') -Raw).
        Replace('{PUBLISHER}', $Publisher).Replace('{VERSION}', "$version.0").Replace('{ARCH}', $Arch)
    Set-Content -Path (Join-Path $layout 'AppxManifest.xml') -Value $manifest -Encoding utf8

    Add-Type -AssemblyName System.Drawing
    foreach ($logo in @(@{ Name = 'StoreLogo'; Size = 50 }, @{ Name = 'Square44x44Logo'; Size = 44 }, @{ Name = 'Square150x150Logo'; Size = 150 })) {
        $size = $logo.Size
        $bitmap = New-Object System.Drawing.Bitmap $size, $size
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.SmoothingMode = 'AntiAlias'
        $radius = [int]($size * 0.22)
        $path = New-Object System.Drawing.Drawing2D.GraphicsPath
        $path.AddArc(0, 0, $radius * 2, $radius * 2, 180, 90)
        $path.AddArc($size - $radius * 2 - 1, 0, $radius * 2, $radius * 2, 270, 90)
        $path.AddArc($size - $radius * 2 - 1, $size - $radius * 2 - 1, $radius * 2, $radius * 2, 0, 90)
        $path.AddArc(0, $size - $radius * 2 - 1, $radius * 2, $radius * 2, 90, 90)
        $graphics.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(0x6E, 0x8B, 0xFF))), $path)
        $ring = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), ($size * 0.12)
        $lens = $size * 0.48
        $graphics.DrawEllipse($ring, ($size - $lens) / 2, ($size - $lens) / 2, $lens, $lens)
        $bitmap.Save((Join-Path $layout "Assets\$($logo.Name).png"), [System.Drawing.Imaging.ImageFormat]::Png)
        $graphics.Dispose(); $bitmap.Dispose()
    }

    if ($Mode -eq 'Register') {
        Get-AppxPackage -Name WebCa | Remove-AppxPackage
        Add-AppxPackage -Register (Join-Path $layout 'AppxManifest.xml')
        $package = Get-AppxPackage -Name WebCa
        "Registered $($package.PackageFullName)"
        "Start it from the Start menu or with: explorer.exe shell:AppsFolder\$($package.PackageFamilyName)!App"
    } else {
        $makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter makeappx.exe |
            Where-Object FullName -like "*\x64\*" | Sort-Object FullName -Descending | Select-Object -First 1
        if (-not $makeappx) { throw 'makeappx.exe was not found; install the Windows SDK.' }
        $output = Join-Path $root "target\package\WebCa-$version-$Arch.msix"
        & $makeappx.FullName pack /o /d $layout /p $output
        if ($LASTEXITCODE -ne 0) { throw 'makeappx failed' }
        "Packed $output"
    }
} finally {
    Pop-Location
}

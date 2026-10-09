param(
    [string]$InputPath = (Join-Path $PSScriptRoot 'output\mnemosyne-os-pitch.pptx'),
    [string]$PdfPath = (Join-Path $PSScriptRoot 'output\mnemosyne-os-pitch.pdf'),
    [string]$PreviewDirectory = (Join-Path $PSScriptRoot '..\build\pitch\powerpoint')
)

# Optional Windows export helper. Requires installed desktop PowerPoint.
# Opens this deck read-only and never saves changes to the source PPTX.
$ErrorActionPreference = 'Stop'
$sourcePath = (Resolve-Path -LiteralPath $InputPath).Path
$destinationPath = [System.IO.Path]::GetFullPath($PdfPath)
$previewPath = [System.IO.Path]::GetFullPath($PreviewDirectory)
if (Test-Path -LiteralPath $destinationPath) {
    throw "Output already exists. Choose a new -PdfPath: $destinationPath"
}
New-Item -ItemType Directory -Force -Path ([System.IO.Path]::GetDirectoryName($destinationPath)) | Out-Null
New-Item -ItemType Directory -Force -Path $previewPath | Out-Null
$hadPowerPoint = @(Get-Process POWERPNT -ErrorAction SilentlyContinue).Count -gt 0
$powerPoint = $null
$deck = $null
try {
    $powerPoint = New-Object -ComObject PowerPoint.Application
    # msoTrue = -1; msoFalse = 0. WithWindow=false avoids opening an editing window.
    $deck = $powerPoint.Presentations.Open($sourcePath, -1, 0, 0)
    $slideCount = $deck.Slides.Count
    for ($index = 1; $index -le $slideCount; $index++) {
        $slide = $deck.Slides.Item($index)
        try {
            $slide.Export((Join-Path $previewPath ('slide-{0:D2}.png' -f $index)), 'PNG', 1280, 720)
        } finally {
            [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($slide)
        }
    }
    # ppSaveAsPDF = 32. The presentation is closed below without saving its PPTX.
    $deck.SaveAs($destinationPath, 32)
    [pscustomobject]@{
        source = $sourcePath
        pdf = $destinationPath
        native_preview_directory = $previewPath
        slides = $slideCount
        renderer = 'Microsoft PowerPoint'
    } | ConvertTo-Json
} finally {
    if ($null -ne $deck) {
        $deck.Close()
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck)
    }
    if ($null -ne $powerPoint) {
        # Do not close a pre-existing PowerPoint session or any other presentation.
        if (-not $hadPowerPoint -and $powerPoint.Presentations.Count -eq 0) {
            $powerPoint.Quit()
        }
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($powerPoint)
    }
}

# Downloads models of the stage B1 gate on Windows (bench/b1/models.txt). A broken download resumes where it
# stopped; every file is checked against its SHA-256 before it is used.
#
#   powershell -ExecutionPolicy Bypass -File bench\b1\download.ps1 -Dir D:\models [-Model 70b]
#
# -Model: 8b, 32b, 70b or all (default 70b: the Windows computer of the ring needs only the 70B model).
# -List: another list in the format of models.txt.
param(
    [Parameter(Mandatory = $true)][string]$Dir,
    [ValidateSet("8b", "32b", "70b", "all")][string]$Model = "70b",
    [string]$List = (Join-Path $PSScriptRoot "models.txt")
)
$ErrorActionPreference = "Stop"
$list = $List
New-Item -ItemType Directory -Force -Path $Dir | Out-Null

$entries = Get-Content $list | Where-Object { $_ -and -not $_.StartsWith("#") } | ForEach-Object {
    $f = $_ -split "\s+"
    [pscustomobject]@{ Repo = $f[0]; File = $f[1]; Bytes = [int64]$f[2]; Sha = $f[3] }
} | Where-Object { $Model -eq "all" -or $_.File -match ($Model.ToUpper()) }

$need = 0
foreach ($e in $entries) {
    $target = Join-Path $Dir $e.File
    $have = if (Test-Path $target) { (Get-Item $target).Length } else { 0 }
    $need += $e.Bytes - $have
}
$drive = (Get-Item $Dir).PSDrive
if ($need -gt $drive.Free) {
    throw ("not enough disk space on {0}: {1:N0} GB more needed, {2:N0} GB free" -f $drive.Name, ($need / 1e9), ($drive.Free / 1e9))
}

$status = 0
foreach ($e in $entries) {
    $target = Join-Path $Dir $e.File
    if ((Test-Path $target) -and (Get-Item $target).Length -eq $e.Bytes) {
        if ((Get-FileHash -Algorithm SHA256 $target).Hash.ToLower() -eq $e.Sha) { Write-Host "ok       $($e.File)"; continue }
        Write-Host "corrupt  $($e.File): downloading it again"
        Remove-Item $target
    }
    Write-Host ("fetching {0} ({1:N1} GB) from huggingface.co/{2}" -f $e.File, ($e.Bytes / 1e9), $e.Repo)
    # curl.exe ships with Windows 10 and 11; -C - resumes a partial file
    & curl.exe -fL -# --create-dirs --retry 20 --retry-delay 5 --retry-all-errors -C - -o $target "https://huggingface.co/$($e.Repo)/resolve/main/$($e.File)"
    if ($LASTEXITCODE -ne 0) { Write-Host "BAD      $($e.File): the download failed; run again to resume"; $status = 1; continue }
    if ((Get-FileHash -Algorithm SHA256 $target).Hash.ToLower() -eq $e.Sha) {
        Write-Host "ok       $($e.File)"
    } else {
        Write-Host "BAD      $($e.File): the SHA-256 does not match; delete it and run again"
        $status = 1
    }
}
exit $status

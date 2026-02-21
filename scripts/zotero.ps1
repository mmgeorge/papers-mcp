# Zotero API explorer
# From PowerShell: .\zotero.ps1 /users/16916553/items/top?limit=2
# From Git Bash:   MSYS_NO_PATHCONV=1 powershell -File zotero.ps1 /users/16916553/items/top?limit=2
# Examples:
#   /users/16916553/items/top?limit=2
#   /users/16916553/fulltext?since=0
#   /users/16916553/deleted?since=0
#   /users/16916553/settings/tagColors
#   /keys/current

param(
    [Parameter(Mandatory)][string]$Path
)

$key = [System.Environment]::GetEnvironmentVariable('ZOTERO_API_KEY', 'User')
$headers = @{
    'Zotero-API-Version' = '3'
    'Zotero-API-Key' = $key
}
$url = "https://api.zotero.org$Path"

Write-Host "GET $url" -ForegroundColor Cyan

try {
    $resp = Invoke-WebRequest -Uri $url -Headers $headers -UseBasicParsing
} catch {
    $resp = $_.Exception.Response
    Write-Host "Status: $($resp.StatusCode.value__)" -ForegroundColor Red
    exit 1
}

Write-Host "Status: $($resp.StatusCode)" -ForegroundColor Green

# Print relevant headers
$interestingHeaders = @('Total-Results', 'Last-Modified-Version', 'Link', 'Content-Type')
foreach ($h in $interestingHeaders) {
    if ($resp.Headers[$h]) {
        Write-Host "$h`: $($resp.Headers[$h])" -ForegroundColor Yellow
    }
}

Write-Host ""

# Pretty-print if JSON, else raw
try {
    $resp.Content | ConvertFrom-Json | ConvertTo-Json -Depth 10
} catch {
    $resp.Content
}

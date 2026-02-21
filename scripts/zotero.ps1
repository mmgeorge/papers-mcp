# Zotero API explorer
# Uses ZOTERO_TEST_API_KEY and ZOTERO_TEST_USER_ID.
# The path is relative to /users/<id> — no need to include the prefix.
#
# Usage (PowerShell):
#   .\scripts\zotero.ps1 /items/top?limit=2
#   .\scripts\zotero.ps1 /fulltext?since=0
#   .\scripts\zotero.ps1 /deleted?since=0
#   .\scripts\zotero.ps1 /settings/tagColors
#   .\scripts\zotero.ps1 /keys/current        # not user-scoped, still works
#
# Usage (Git Bash):
#   MSYS_NO_PATHCONV=1 powershell -File scripts/zotero.ps1 /items/top?limit=2

param(
    [Parameter(Mandatory)][string]$Path
)

$key    = [System.Environment]::GetEnvironmentVariable('ZOTERO_TEST_API_KEY',  'User')
$userId = [System.Environment]::GetEnvironmentVariable('ZOTERO_TEST_USER_ID', 'User')

if (-not $key -or -not $userId) {
    Write-Error "ZOTERO_TEST_API_KEY and ZOTERO_TEST_USER_ID must be set in user environment"
    exit 1
}

$headers = @{
    'Zotero-API-Version' = '3'
    'Zotero-API-Key'     = $key
}

# Non-user-scoped paths (e.g. /keys/current) start with /keys
$url = if ($Path -like '/keys/*') {
    "https://api.zotero.org$Path"
} else {
    "https://api.zotero.org/users/$userId$Path"
}

Write-Host "GET $url" -ForegroundColor Cyan

try {
    $resp = Invoke-WebRequest -Uri $url -Headers $headers -UseBasicParsing
} catch {
    $resp = $_.Exception.Response
    Write-Host "Status: $($resp.StatusCode.value__)" -ForegroundColor Red
    exit 1
}

Write-Host "Status: $($resp.StatusCode)" -ForegroundColor Green

$interestingHeaders = @('Total-Results', 'Last-Modified-Version', 'Link', 'Content-Type')
foreach ($h in $interestingHeaders) {
    if ($resp.Headers[$h]) {
        Write-Host "${h}: $($resp.Headers[$h])" -ForegroundColor Yellow
    }
}

Write-Host ""

try {
    $resp.Content | ConvertFrom-Json | ConvertTo-Json -Depth 10
} catch {
    $resp.Content
}

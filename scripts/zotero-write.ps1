# Zotero write API explorer
# Tests all write endpoints: create, update, patch, delete for items,
# collections, and searches; and delete for tags.
#
# Uses ZOTERO_TEST_API_KEY and ZOTERO_TEST_USER_ID (dedicated test library).
#
# Usage (from repo root, PowerShell):
#   .\scripts\zotero-write.ps1
#
# Usage (Git Bash):
#   powershell -File scripts/zotero-write.ps1

$key     = [System.Environment]::GetEnvironmentVariable('ZOTERO_TEST_API_KEY',  'User')
$userId  = [System.Environment]::GetEnvironmentVariable('ZOTERO_TEST_USER_ID', 'User')

if (-not $key -or -not $userId) {
    Write-Error "ZOTERO_TEST_API_KEY and ZOTERO_TEST_USER_ID must be set in user environment"
    exit 1
}

$base    = "https://api.zotero.org/users/$userId"
$headers = @{
    'Zotero-API-Version' = '3'
    'Zotero-API-Key'     = $key
    'Content-Type'       = 'application/json'
}

function Invoke-Zotero {
    param(
        [string]$Method,
        [string]$Path,
        [string]$Body = $null,
        [hashtable]$ExtraHeaders = @{}
    )
    $url = "$base$Path"
    $h = $headers.Clone()
    foreach ($k in $ExtraHeaders.Keys) { $h[$k] = $ExtraHeaders[$k] }

    Write-Host "$Method $url" -ForegroundColor Cyan
    if ($Body) { Write-Host "Body: $Body" -ForegroundColor DarkGray }

    $params = @{ Uri = $url; Method = $Method; Headers = $h; UseBasicParsing = $true }
    if ($Body) { $params['Body'] = $Body }

    try {
        $resp = Invoke-WebRequest @params
        Write-Host "Status: $($resp.StatusCode)" -ForegroundColor Green
        $lmv = $resp.Headers['Last-Modified-Version']
        if ($lmv) { Write-Host "Last-Modified-Version: $lmv" -ForegroundColor Yellow }
        return $resp
    } catch {
        $resp = $_.Exception.Response
        $status = if ($resp) { $resp.StatusCode.value__ } else { '???' }
        Write-Host "Status: $status  ERROR: $($_.Exception.Message)" -ForegroundColor Red
        return $null
    }
}

function Print-Json($text) {
    try { $text | ConvertFrom-Json | ConvertTo-Json -Depth 10 }
    catch { Write-Host $text }
}

# ── Get current library version ───────────────────────────────────────
Write-Host "`n=== Get library version ===" -ForegroundColor Magenta
$r = Invoke-Zotero -Method GET -Path "/items?limit=0"
$libVersion = if ($r) { [int]$r.Headers['Last-Modified-Version'] } else { 0 }
Write-Host "Library version: $libVersion"

# ── Create a note item ────────────────────────────────────────────────
Write-Host "`n=== Create note item ===" -ForegroundColor Magenta
$createBody = '[{"itemType":"note","note":"<p>Test note created by zotero-write.ps1</p>","tags":[{"tag":"test-write"}]}]'
$r = Invoke-Zotero -Method POST -Path "/items" -Body $createBody
if ($r) {
    Print-Json $r.Content
    $result = $r.Content | ConvertFrom-Json
    $noteKey     = $result.successful.'0'.key
    $noteVersion = $result.successful.'0'.version
    $libVersion  = [int]$r.Headers['Last-Modified-Version']
    Write-Host "Created note key=$noteKey version=$noteVersion" -ForegroundColor Green
} else {
    Write-Host "Create failed, stopping." -ForegroundColor Red
    exit 1
}

# ── Patch the note (partial update) ──────────────────────────────────
Write-Host "`n=== Patch note item ===" -ForegroundColor Magenta
$patchBody = '{"note":"<p>Updated by PATCH</p>"}'
$r = Invoke-Zotero -Method PATCH -Path "/items/$noteKey" -Body $patchBody `
    -ExtraHeaders @{ 'If-Unmodified-Since-Version' = "$noteVersion" }
if ($r) { $noteVersion++ }

# ── GET the patched item ──────────────────────────────────────────────
Write-Host "`n=== GET patched item ===" -ForegroundColor Magenta
$r = Invoke-Zotero -Method GET -Path "/items/$noteKey"
if ($r) {
    $item = $r.Content | ConvertFrom-Json
    $noteVersion = $item.version
    Write-Host "note content: $($item.data.note)"
}

# ── Create a journal article (tests a richer item type) ───────────────
Write-Host "`n=== Create journal article ===" -ForegroundColor Magenta
$articleBody = @'
[{
    "itemType": "journalArticle",
    "title": "Test Article from zotero-write.ps1",
    "creators": [{"creatorType":"author","firstName":"Test","lastName":"Author"}],
    "tags": [{"tag":"test-write"}],
    "abstractNote": "This item was created by the write test script."
}]
'@
$r = Invoke-Zotero -Method POST -Path "/items" -Body $articleBody
if ($r) {
    Print-Json $r.Content
    $result      = $r.Content | ConvertFrom-Json
    $artKey      = $result.successful.'0'.key
    $artVersion  = $result.successful.'0'.version
    $libVersion  = [int]$r.Headers['Last-Modified-Version']
    Write-Host "Created article key=$artKey version=$artVersion" -ForegroundColor Green
}

# ── PUT (full replace) the article ────────────────────────────────────
Write-Host "`n=== PUT (full replace) article ===" -ForegroundColor Magenta
$putBody = @"
{
    "key": "$artKey",
    "version": $artVersion,
    "itemType": "journalArticle",
    "title": "Updated Title via PUT",
    "creators": [{"creatorType":"author","firstName":"Test","lastName":"Author"}],
    "tags": [{"tag":"test-write"},{"tag":"test-put"}],
    "abstractNote": "Replaced by PUT."
}
"@
$r = Invoke-Zotero -Method PUT -Path "/items/$artKey" -Body $putBody `
    -ExtraHeaders @{ 'If-Unmodified-Since-Version' = "$artVersion" }
if ($r) { $artVersion++ }

# ── Create a collection ───────────────────────────────────────────────
Write-Host "`n=== Create collection ===" -ForegroundColor Magenta
$colBody = '[{"name":"Test Collection from write script"}]'
$r = Invoke-Zotero -Method POST -Path "/collections" -Body $colBody
$colKey     = $null
$colVersion = $null
if ($r) {
    Print-Json $r.Content
    $result     = $r.Content | ConvertFrom-Json
    $colKey     = $result.successful.'0'.key
    $colVersion = $result.successful.'0'.version
    $libVersion = [int]$r.Headers['Last-Modified-Version']
    Write-Host "Created collection key=$colKey version=$colVersion" -ForegroundColor Green
}

# ── Update the collection ─────────────────────────────────────────────
Write-Host "`n=== Update collection ===" -ForegroundColor Magenta
if ($colKey) {
    $updateColBody = @"
{
    "key": "$colKey",
    "version": $colVersion,
    "name": "Updated Collection Name",
    "parentCollection": false
}
"@
    $r = Invoke-Zotero -Method PUT -Path "/collections/$colKey" -Body $updateColBody `
        -ExtraHeaders @{ 'If-Unmodified-Since-Version' = "$colVersion" }
    if ($r) { $colVersion++ }
}

# ── Create a saved search ─────────────────────────────────────────────
Write-Host "`n=== Create saved search ===" -ForegroundColor Magenta
$searchBody = '[{"name":"Test Search from write script","conditions":[{"condition":"tag","operator":"is","value":"test-write"}]}]'
$r = Invoke-Zotero -Method POST -Path "/searches" -Body $searchBody
$searchKey = $null
if ($r) {
    Print-Json $r.Content
    $result     = $r.Content | ConvertFrom-Json
    $searchKey  = $result.successful.'0'.key
    $libVersion = [int]$r.Headers['Last-Modified-Version']
    Write-Host "Created search key=$searchKey" -ForegroundColor Green
}

# ── Delete all test items (note + article) ────────────────────────────
Write-Host "`n=== Delete items (multi) ===" -ForegroundColor Magenta
$keysToDelete = @($noteKey, $artKey) | Where-Object { $_ }
if ($keysToDelete.Count -gt 0) {
    $keyParam = $keysToDelete -join ","
    $r = Invoke-Zotero -Method DELETE -Path "/items?itemKey=$keyParam" `
        -ExtraHeaders @{ 'If-Unmodified-Since-Version' = "$libVersion" }
    if ($r) { $libVersion = [int]$r.Headers['Last-Modified-Version'] }
}

# ── Delete the collection ─────────────────────────────────────────────
Write-Host "`n=== Delete collection ===" -ForegroundColor Magenta
if ($colKey) {
    $r = Invoke-Zotero -Method DELETE -Path "/collections/$colKey" `
        -ExtraHeaders @{ 'If-Unmodified-Since-Version' = "$colVersion" }
    if ($r) { $libVersion = [int]$r.Headers['Last-Modified-Version'] }
}

# ── Delete the saved search ───────────────────────────────────────────
Write-Host "`n=== Delete saved search ===" -ForegroundColor Magenta
if ($searchKey) {
    $r = Invoke-Zotero -Method DELETE -Path "/searches?searchKey=$searchKey" `
        -ExtraHeaders @{ 'If-Unmodified-Since-Version' = "$libVersion" }
    if ($r) { $libVersion = [int]$r.Headers['Last-Modified-Version'] }
}

# ── Delete the test-write tag ─────────────────────────────────────────
Write-Host "`n=== Delete tag 'test-write' ===" -ForegroundColor Magenta
$r = Invoke-Zotero -Method DELETE -Path "/tags?tag=test-write" `
    -ExtraHeaders @{ 'If-Unmodified-Since-Version' = "$libVersion" }

# ── Verify library is empty ───────────────────────────────────────────
Write-Host "`n=== Final state: list all items ===" -ForegroundColor Magenta
$r = Invoke-Zotero -Method GET -Path "/items"
if ($r) { Print-Json $r.Content }

Write-Host "`nDone." -ForegroundColor Green

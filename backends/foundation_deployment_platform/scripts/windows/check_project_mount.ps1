# Check if project mount is accessible and actually readable.
# Returns: "mounted|METHOD" where METHOD is virtiofs, smb, or unknown
# Or: "missing" if no mount detected.
$ErrorActionPreference = 'SilentlyContinue'

$mountPoint = 'C:\Users\vagrant\project'
$smbDrive = 'Z:'

# Check virtiofs mount at C:\Users\vagrant\project
if (Test-Path $mountPoint) {
    try {
        # Try to actually read a file to verify mount is working
        $testFile = Join-Path $mountPoint "Cargo.toml"
        if (Test-Path $testFile) {
            $content = Get-Content $testFile -TotalCount 1
            if ($content -and $content.Contains('[package]')) {
                "mounted|virtiofs"
                exit 0
            }
        }
        # Mount exists but can't read - might be stale
        "exists|unreadable"
        exit 0
    } catch {
        # Exists but not readable
        "exists|error"
        exit 0
    }
}

# Check SMB mount at Z:
if (Test-Path $smbDrive) {
    try {
        $testFile = Join-Path $smbDrive "Cargo.toml"
        if (Test-Path $testFile) {
            $content = Get-Content $testFile -TotalCount 1
            if ($content -and $content.Contains('[package]')) {
                "mounted|smb"
                exit 0
            }
        }
        "exists|unreadable"
        exit 0
    } catch {
        "exists|error"
        exit 0
    }
}

# Check if virtiofs process is running (indicates attempt in progress)
$proc = Get-Process -Name "virtiofs" -ErrorAction SilentlyContinue
if ($proc) {
    "starting|virtiofs"
    exit 0
}

"missing"

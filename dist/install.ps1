# Install Sail on Windows, from one line pasted into PowerShell:
#
#     irm https://raw.githubusercontent.com/kapoost/sail/main/dist/install.ps1 | iex
#
# # Why this exists
#
# Windows refuses to run an executable that nobody has paid for a certificate to vouch
# for. The refusal is a blue box saying "Windows protected your PC", and the button that
# runs it anyway is behind "More info" — which is one more step than most people take
# before deciding a game is broken.
#
# What makes an installer a fix rather than a workaround is the same detail as on the
# Mac: that warning comes from the *mark of the web*, an alternate data stream that the
# program doing the downloading attaches. A browser attaches it. `Invoke-WebRequest` does
# not. So an executable that arrives through here has nothing on it for SmartScreen to
# object to, and it runs by double-clicking like anything else.
#
# Which is also why it is a line you paste rather than a file you download: a .ps1 saved
# from a web page would carry the mark itself, and the execution policy would then refuse
# to run it. Piped into `iex` it is a string in memory, which the policy does not govern.
#
# The real fix is an EV code-signing certificate at a few hundred a year, after which a
# download from a web page simply runs. This is what you do before that is worth it.

$ErrorActionPreference = 'Stop'

$Repo = 'kapoost/sail'
# Not `$Home`: that is one of PowerShell's own automatic variables and means the user's
# profile directory. Assigning to it is read-only in Windows PowerShell and merely
# confusing everywhere else.
$Dest = Join-Path $env:LOCALAPPDATA 'Sail'

function Say($text) { Write-Host $text }

if ($env:OS -ne 'Windows_NT') {
    throw 'This installer is for Windows. On a Mac use install.sh.'
}

Say "Looking for the latest release of $Repo..."
# The release API rather than a fixed URL, so this does not have to be edited every time
# a version goes out — which is the version of this that rots.
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" `
    -Headers @{ 'User-Agent' = 'sail-installer' }
$tag = $release.tag_name
$game = $release.assets | Where-Object { $_.name -eq 'Sail.exe' } | Select-Object -First 1
$relay = $release.assets | Where-Object { $_.name -eq 'sail-server.exe' } | Select-Object -First 1
if (-not $game) { throw "Release $tag has no Windows build in it yet. Try again in an hour." }
Say "Found $tag."

New-Item -ItemType Directory -Force -Path $Dest | Out-Null
$exe = Join-Path $Dest 'Sail.exe'

Say 'Downloading...'
Invoke-WebRequest -Uri $game.browser_download_url -OutFile $exe -UseBasicParsing
if ($relay) {
    Invoke-WebRequest -Uri $relay.browser_download_url `
        -OutFile (Join-Path $Dest 'sail-server.exe') -UseBasicParsing
}

# Belt and braces. Nothing fetched this way carries the mark of the web, but a copy the
# person already pulled down with a browser may have left one behind in this folder.
Get-ChildItem -Path $Dest -Filter *.exe | Unblock-File

# A Start menu entry, so it is launched the way everything else on the machine is rather
# than by remembering a path. Nothing is written outside this user's own profile and no
# administrator rights are asked for.
$menu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Sail.lnk'
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($menu)
$link.TargetPath = $exe
$link.WorkingDirectory = $Dest
$link.Description = 'A sailing simulator'
$link.Save()

Say ''
Say "Sail $tag is in $Dest, and in the Start menu."
Say "    & '$exe'"
if ($relay) {
    Say ''
    Say 'To host a game for your friends:'
    Say "    & '$(Join-Path $Dest 'sail-server.exe')' --seed 42"
    Say 'and they join from the game's own menu: Esc, then TOGETHER.'
}
Say ''
Say 'Run this line again any time to get the newest version.'

param(
  [Parameter(Mandatory = $true)]
  [string]$Executable,
  [Parameter(Mandatory = $true)]
  [string]$Mkvmerge,
  [Parameter(Mandatory = $true)]
  [string]$Fixture,
  [Parameter(Mandatory = $true)]
  [string]$WorkDirectory
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class InstalledCloseSmokeNativeMethods {
    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    public static extern IntPtr SendMessage(IntPtr hWnd, UInt32 message, IntPtr wParam, IntPtr lParam);
}
"@

function Get-AccessibleWindow([System.Diagnostics.Process]$Process) {
  $Process.Refresh()
  if ($Process.HasExited -or $Process.MainWindowHandle -eq [IntPtr]::Zero) {
    return $null
  }
  return [System.Windows.Automation.AutomationElement]::FromHandle($Process.MainWindowHandle)
}

function Find-Button(
  [System.Windows.Automation.AutomationElement]$Window,
  [string]$Name
) {
  $condition = [System.Windows.Automation.AndCondition]::new(
    [System.Windows.Automation.PropertyCondition]::new(
      [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
      [System.Windows.Automation.ControlType]::Button
    ),
    [System.Windows.Automation.PropertyCondition]::new(
      [System.Windows.Automation.AutomationElement]::NameProperty,
      $Name
    )
  )
  return $Window.FindFirst(
    [System.Windows.Automation.TreeScope]::Descendants,
    $condition
  )
}

function Wait-ForButton(
  [System.Diagnostics.Process]$Process,
  [string]$Name,
  [int]$TimeoutSeconds = 60
) {
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    $window = Get-AccessibleWindow $Process
    if ($null -ne $window) {
      $button = Find-Button $window $Name
      if ($null -ne $button) { return $button }
    }
    Start-Sleep -Milliseconds 500
  }
  throw "The installed app did not expose its '$Name' button within $TimeoutSeconds seconds."
}

function Wait-ForConfirmation([int]$TimeoutSeconds = 30) {
  $desktop = [System.Windows.Automation.AutomationElement]::RootElement
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    $windows = $desktop.FindAll(
      [System.Windows.Automation.TreeScope]::Children,
      [System.Windows.Automation.Condition]::TrueCondition
    )
    foreach ($candidate in $windows) {
      if ($candidate.Current.Name -ne "Muxing in progress") { continue }
      $yes = Find-Button $candidate "Yes"
      if ($null -ne $yes) { return $yes }
    }
    Start-Sleep -Milliseconds 250
  }
  throw "The native 'Muxing in progress' confirmation did not appear after closing the window."
}

function Close-ThroughNativeWindow([System.Diagnostics.Process]$Process) {
  $Process.Refresh()
  if ($Process.HasExited -or $Process.MainWindowHandle -eq [IntPtr]::Zero) {
    throw "The installed app exited or lost its window before the close check."
  }
  [void][InstalledCloseSmokeNativeMethods]::SendMessage(
    $Process.MainWindowHandle,
    0x0010,
    [IntPtr]::Zero,
    [IntPtr]::Zero
  )
}

function Assert-NoPartialOutputs([string]$OutputDirectory) {
  $partials = @(Get-ChildItem -LiteralPath $OutputDirectory -Recurse -Force -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like ".*.partial.mkv" })
  if ($partials.Count -gt 0) {
    throw "The installed app left staged partial output(s): $($partials.FullName -join ', ')"
  }
}

function Assert-ValidMkvOutputs([string]$OutputDirectory, [int]$ExpectedCount) {
  $outputs = @(Get-ChildItem -LiteralPath $OutputDirectory -Recurse -File -Filter "*.mkv" |
    Where-Object { $_.Name -notlike ".*.partial.mkv" })
  if ($outputs.Count -ne $ExpectedCount) {
    throw "Expected $ExpectedCount finalized MKV output(s), found $($outputs.Count)."
  }
  foreach ($output in $outputs) {
    $probe = & $Mkvmerge -J $output.FullName 2>&1
    if ($LASTEXITCODE -ne 0) {
      throw "mkvmerge could not inspect finalized output '$($output.FullName)': $probe"
    }
    $metadata = ($probe | Out-String) | ConvertFrom-Json
    if ($metadata.container.type -notmatch "Matroska" -or $metadata.tracks.Count -lt 1) {
      throw "Finalized output '$($output.FullName)' is not a Matroska file with a track."
    }
  }
}

if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
  throw "The installed application executable is missing: $Executable"
}
if (-not (Test-Path -LiteralPath $Fixture -PathType Leaf)) {
  throw "The packaged remux fixture is missing: $Fixture"
}

foreach ($path in @(
  (Join-Path $WorkDirectory "app-data"),
  (Join-Path $WorkDirectory "inputs"),
  (Join-Path $WorkDirectory "outputs"),
  (Join-Path $WorkDirectory "mkvmerge-started.log"),
  (Join-Path $WorkDirectory "delayed-mkvmerge.rs"),
  (Join-Path $WorkDirectory "delayed-mkvmerge.exe")
)) {
  Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Path $WorkDirectory -Force | Out-Null
$configRoot = Join-Path $WorkDirectory "app-data"
$configDirectory = Join-Path $configRoot "bdmv-remuxer"
$inputDirectory = Join-Path $WorkDirectory "inputs"
$outputDirectory = Join-Path $WorkDirectory "outputs"
$marker = Join-Path $WorkDirectory "mkvmerge-started.log"
$wrapperSource = Join-Path $WorkDirectory "delayed-mkvmerge.rs"
$wrapper = Join-Path $WorkDirectory "delayed-mkvmerge.exe"
$workspacePath = Join-Path $configDirectory "workspace.json"
$settingsPath = Join-Path $configDirectory "settings.json"
New-Item -ItemType Directory -Path $configDirectory, $inputDirectory, $outputDirectory -Force | Out-Null

$firstInput = Join-Path $inputDirectory "first.m2ts"
$secondInput = Join-Path $inputDirectory "second.m2ts"
Copy-Item -LiteralPath $Fixture -Destination $firstInput
Copy-Item -LiteralPath $Fixture -Destination $secondInput

@"
use std::{env, fs, process::Command, thread, time::Duration};

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if let Err(error) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(r#"$marker"#)
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "started")
        })
    {
        eprintln!("Could not record delayed mux start: {error}");
        std::process::exit(2);
    }
    thread::sleep(Duration::from_secs(12));
    let status = match Command::new(r#"$Mkvmerge"#).args(args).status() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("Could not start real mkvmerge: {error}");
            std::process::exit(2);
        }
    };
    std::process::exit(status.code().unwrap_or(1));
}
"@ | Set-Content -LiteralPath $wrapperSource -Encoding UTF8
& rustc $wrapperSource -o $wrapper
if ($LASTEXITCODE -ne 0) {
  throw "Could not build the delayed mkvmerge test wrapper."
}

$files = @(
  [ordered]@{
    id = "close-smoke-first"
    filename = "first.m2ts"
    path = $firstInput
    duration = 1.0
    size = (Get-Item -LiteralPath $firstInput).Length
    category = "EPISODE"
    tracks = @()
    episode_number = 1
    op_number = $null
    ed_number = $null
    special_number = $null
    chapter_file = $null
  },
  [ordered]@{
    id = "close-smoke-second"
    filename = "second.m2ts"
    path = $secondInput
    duration = 1.0
    size = (Get-Item -LiteralPath $secondInput).Length
    category = "EPISODE"
    tracks = @()
    episode_number = 2
    op_number = $null
    ed_number = $null
    special_number = $null
    chapter_file = $null
  }
)
$project = [ordered]@{
  id = "installed-close-smoke"
  output_name = "Installed close smoke"
  output_path = $outputDirectory
  volumes = @(
    [ordered]@{
      id = "installed-close-smoke-volume"
      name = "Close smoke"
      volume_number = 1
      files = $files
      probe_failures = @()
    }
  )
  track_templates = @{}
  status = "pending"
  progress = 0
  current_file = $null
  completed_files = @()
  created_at = [DateTime]::UtcNow.ToString("o")
  error = $null
}
$workspace = [ordered]@{
  current_volumes = @()
  queue = @($project)
  track_templates = @{}
}
$workspace | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $workspacePath -Encoding UTF8
@{ mkvmerge_path = $wrapper; ffprobe_path = "ffprobe" } |
  ConvertTo-Json | Set-Content -LiteralPath $settingsPath -Encoding UTF8

$originalAppData = $env:APPDATA
$env:APPDATA = $configRoot
$firstProcess = $null
$resumeProcess = $null
try {
  $firstProcess = Start-Process -FilePath $Executable -PassThru
  $start = Wait-ForButton $firstProcess "Start"
  ([System.Windows.Automation.InvokePattern]$start.GetCurrentPattern(
    [System.Windows.Automation.InvokePattern]::Pattern
  )).Invoke()

  $startDeadline = [DateTime]::UtcNow.AddSeconds(90)
  while (-not (Test-Path -LiteralPath $marker) -and [DateTime]::UtcNow -lt $startDeadline) {
    $firstProcess.Refresh()
    if ($firstProcess.HasExited) {
      throw "The installed app exited before the delayed remux started (code $($firstProcess.ExitCode))."
    }
    Start-Sleep -Milliseconds 250
  }
  if (-not (Test-Path -LiteralPath $marker)) {
    throw "The installed app did not start the first delayed remux."
  }
  if ((Get-Content -LiteralPath $marker).Count -ne 1) {
    throw "The installed app started another queued file before the close request."
  }

  Close-ThroughNativeWindow $firstProcess
  $yes = Wait-ForConfirmation
  ([System.Windows.Automation.InvokePattern]$yes.GetCurrentPattern(
    [System.Windows.Automation.InvokePattern]::Pattern
  )).Invoke()

  Start-Sleep -Seconds 2
  $firstProcess.Refresh()
  if ($firstProcess.HasExited) {
    throw "The installed app closed before the delayed active file could finish."
  }

  if (-not $firstProcess.WaitForExit(60000)) {
    throw "The installed app did not close after the active remux completed."
  }
  $firstProcess.Refresh()
  if ($firstProcess.ExitCode -ne 0) {
    throw "The installed app exited with code $($firstProcess.ExitCode) during close."
  }

  $savedWorkspace = Get-Content -LiteralPath $workspacePath -Raw | ConvertFrom-Json
  $savedProject = $savedWorkspace.queue | Where-Object { $_.id -eq "installed-close-smoke" }
  if ($null -eq $savedProject -or $savedProject.status -ne "paused") {
    throw "The installed app did not persist the unfinished queue as Paused."
  }
  if ($savedProject.completed_files.Count -ne 1 -or $savedProject.completed_files[0].path -ne $firstInput) {
    throw "The installed app did not save exactly the completed first file."
  }
  Assert-ValidMkvOutputs $outputDirectory 1
  Assert-NoPartialOutputs $outputDirectory
  Write-Host "Native close waited for the active file, finalized its output, and saved the remaining file as Paused."

  $resumeProcess = Start-Process -FilePath $Executable -PassThru
  $resumeButton = Wait-ForButton $resumeProcess "Start"
  ([System.Windows.Automation.InvokePattern]$resumeButton.GetCurrentPattern(
    [System.Windows.Automation.InvokePattern]::Pattern
  )).Invoke()

  $resumeDeadline = [DateTime]::UtcNow.AddSeconds(120)
  $resumed = $false
  while ([DateTime]::UtcNow -lt $resumeDeadline) {
    $resumeProcess.Refresh()
    if ($resumeProcess.HasExited) {
      throw "The reopened app exited before the paused queue completed (code $($resumeProcess.ExitCode))."
    }
    $currentWorkspace = Get-Content -LiteralPath $workspacePath -Raw | ConvertFrom-Json
    $currentProject = $currentWorkspace.queue | Where-Object { $_.id -eq "installed-close-smoke" }
    if ($null -ne $currentProject -and $currentProject.status -eq "complete") {
      $resumed = $true
      break
    }
    Start-Sleep -Milliseconds 500
  }
  if (-not $resumed) {
    throw "The reopened app did not resume and complete the remaining queued file."
  }
  if ((Get-Content -LiteralPath $marker).Count -ne 2) {
    throw "The reopened app did not run exactly the one remaining file."
  }
  $completedWorkspace = Get-Content -LiteralPath $workspacePath -Raw | ConvertFrom-Json
  $completedProject = $completedWorkspace.queue | Where-Object { $_.id -eq "installed-close-smoke" }
  if ($completedProject.completed_files.Count -ne 2) {
    throw "The reopened app did not persist both source files as completed."
  }
  Assert-ValidMkvOutputs $outputDirectory 2
  Assert-NoPartialOutputs $outputDirectory

  Close-ThroughNativeWindow $resumeProcess
  if (-not $resumeProcess.WaitForExit(30000)) {
    throw "The reopened installed app did not close after the resumed queue completed."
  }
  Write-Host "The reopened app resumed the remaining file; both outputs are valid and no partial output remains."
}
finally {
  foreach ($process in @($firstProcess, $resumeProcess)) {
    if ($null -ne $process) {
      $process.Refresh()
      if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
      }
    }
  }
  $env:APPDATA = $originalAppData
}

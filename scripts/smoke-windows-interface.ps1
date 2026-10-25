param(
  [switch]$TestMissingHeading
)

function Get-MissingInterfaceHeadingDiagnostic {
  param(
    [Parameter(Mandatory = $true)]
    [bool]$InterfaceRendered
  )

  if ($InterfaceRendered) {
    return $null
  }

  return "The installed application opened a native window, but its rendered main interface heading was not found. The webview may be blank or failed to load."
}

if ($TestMissingHeading) {
  $diagnostic = Get-MissingInterfaceHeadingDiagnostic -InterfaceRendered:$false
  [Console]::Error.WriteLine($diagnostic)
  exit 3
}

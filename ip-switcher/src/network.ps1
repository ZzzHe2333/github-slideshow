$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Enc([object]$value) { [Uri]::EscapeDataString([string]$value) }

function Get-Wired {
  @(Get-NetAdapter -Physical -ErrorAction Stop | Where-Object {
    $m = "$($_.MediaType) $($_.PhysicalMediaType)"
    $d = [string]$_.InterfaceDescription
    $_.HardwareInterface -and
    $m -notmatch '(?i)wireless|wi-?fi|802\.11|bluetooth' -and
    $d -notmatch '(?i)wireless|wi-?fi|802\.11|bluetooth' -and
    ($m -match '(?i)ethernet|802\.3' -or $d -match '(?i)ethernet|\bgbe\b|\bi219\b|\bi225\b|\bi226\b')
  } | Sort-Object -Property InterfaceIndex)
}
function AddressRows([uint32]$idx) {
  @(Get-NetIPAddress -InterfaceIndex $idx -AddressFamily IPv4 -ErrorAction Stop |
    Where-Object { $_.IPAddress -ne '0.0.0.0' -and $_.IPAddress -notlike '169.254.*' })
}
function Dns([uint32]$idx) {
  $dnsObj = Get-DnsClientServerAddress -InterfaceIndex $idx -AddressFamily IPv4 -ErrorAction Stop
  (@($dnsObj.ServerAddresses) -join ', ')
}
function Gateway([uint32]$idx) {
  $routes = @(Get-NetRoute -InterfaceIndex $idx -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue)
  (@($routes | ForEach-Object { $_.NextHop } | Sort-Object -Unique) -join ', ')
}
function AsNum([string]$ip) {
  $addr = [Net.IPAddress]::Parse($ip)
  if ($addr.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork) { throw '只允许 IPv4。' }
  $b = $addr.GetAddressBytes()
  [uint64]$result = [uint64]$b[0]*16777216 + [uint64]$b[1]*65536 + [uint64]$b[2]*256 + [uint64]$b[3]
  return $result
}

function Invoke-WiredSwitch([string]$Action, [uint32]$Index) {
  $adapters = @(Get-Wired)
  if ($Action -eq 'list') {
    foreach($adapter in $adapters) {
      $idx = [uint32]$adapter.InterfaceIndex
      try {
        $rows = @(AddressRows $idx)
        $ipv4 = if ($rows.Count -eq 1) { [string]$rows[0].IPAddress } elseif ($rows.Count -gt 1) { 'MULTIPLE' } else { '-' }
        $prefix = if ($rows.Count -eq 1) { [string]$rows[0].PrefixLength } else { '-' }
        $dhcp = [string](Get-NetIPInterface -InterfaceIndex $idx -AddressFamily IPv4 -ErrorAction Stop).Dhcp
        Write-Output "ADAPTER|$idx|$(Enc $adapter.Name)|$ipv4|$prefix|$(Enc (Gateway $idx))|$(Enc (Dns $idx))|$dhcp|$(Enc $adapter.Status)"
      } catch {
        Write-Output "ADAPTER|$idx|$(Enc $adapter.Name)|-| -|$(Enc '')|$(Enc $_.Exception.Message)|Unknown|$(Enc $adapter.Status)"
      }
    }
    return
  }
  if ($Action -ne 'switch') { throw '无效操作。' }
  $selected = @($adapters | Where-Object { [uint32]$_.InterfaceIndex -eq $Index })
  if ($selected.Count -ne 1) { throw '已选网卡不是实体有线网卡，拒绝修改。' }
  if ([string]$selected[0].Status -ne 'Up') { throw '有线网卡未连接或已禁用。' }
  $iface = Get-NetIPInterface -InterfaceIndex $Index -AddressFamily IPv4 -ErrorAction Stop
  if ([string]$iface.Dhcp -ne 'Disabled') { throw 'DHCP 模式下禁止修改，以免更改其他设置。' }
  $rows = @(AddressRows $Index)
  if ($rows.Count -ne 1) { throw '有线网卡不是恰好一个静态 IPv4，拒绝修改。' }
  $oldRow = $rows[0]
  if ([string]$oldRow.PrefixOrigin -ne 'Manual') { throw '当前 IPv4 不是手工静态地址。' }
  $old = [string]$oldRow.IPAddress
  if ($old -eq '192.168.0.192') { $target = '192.168.0.193' }
  elseif ($old -eq '192.168.0.193') { $target = '192.168.0.192' }
  else { throw "当前 IP $old 不是 .192 或 .193，禁止修改。" }
  [int]$prefix = $oldRow.PrefixLength
  if ($prefix -lt 8 -or $prefix -gt 30) { throw "不支持的子网前缀 /$prefix。" }
  $subnetSize = [math]::Pow(2,32-$prefix)
  if ([math]::Floor((AsNum $old)/$subnetSize) -ne [math]::Floor((AsNum $target)/$subnetSize)) {
    throw '新旧地址不在同一子网。'
  }
  $hostPart = (AsNum $target) % $subnetSize
  if ($hostPart -eq 0 -or $hostPart -eq ($subnetSize-1)) { throw '目标是网络地址或广播地址。' }
  $inUse = @(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction Stop | Where-Object { $_.IPAddress -eq $target })
  if ($inUse.Count -gt 0) { throw '目标 IP 已被本机使用。' }
  if (Test-Connection -ComputerName $target -Count 1 -Quiet -ErrorAction SilentlyContinue) {
    throw '目标 IP 有设备响应，可能冲突。'
  }
  $beforeGw = Gateway $Index
  $beforeDns = Dns $Index
  $created = $false
  try {
    New-NetIPAddress -InterfaceIndex $Index -IPAddress $target -PrefixLength $prefix -SkipAsSource:([bool]$oldRow.SkipAsSource) -ErrorAction Stop | Out-Null
    $created = $true
    $ready = $false
    for ($i=0; $i -lt 25; $i++) {
      $nr = Get-NetIPAddress -InterfaceIndex $Index -AddressFamily IPv4 -IPAddress $target -ErrorAction Stop
      if ([string]$nr.AddressState -eq 'Preferred') { $ready = $true; break }
      if ([string]$nr.AddressState -in @('Duplicate','Invalid')) { throw '目标 IP 发生地址冲突。' }
      Start-Sleep -Milliseconds 400
    }
    if (-not $ready) { throw '目标 IP 未通过地址冲突检测。' }
    if ((Gateway $Index) -ne $beforeGw -or (Dns $Index) -ne $beforeDns) {
      throw '发现 DNS 或网关变化，已取消操作。'
    }
    Remove-NetIPAddress -InterfaceIndex $Index -IPAddress $old -AddressFamily IPv4 -Confirm:$false -ErrorAction Stop
  } catch {
    $reason = $_.Exception.Message
    if ($created) {
      $stillOld = @(AddressRows $Index | Where-Object { $_.IPAddress -eq $old })
      if ($stillOld.Count -gt 0) {
        Remove-NetIPAddress -InterfaceIndex $Index -IPAddress $target -AddressFamily IPv4 -Confirm:$false -ErrorAction SilentlyContinue
      }
    }
    throw $reason
  }
  $final = @(AddressRows $Index)
  if ($final.Count -ne 1 -or [string]$final[0].IPAddress -ne $target) {
    throw '切换完成，但 IP 校验失败，请检查 Windows 网络设置。'
  }
  if ((Gateway $Index) -ne $beforeGw -or (Dns $Index) -ne $beforeDns) {
    throw '警告：切换完成，但网关或 DNS 状态发生变化。'
  }
  Write-Output "SUCCESS|$old|$target"
}

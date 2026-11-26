# Screenshot a screen region including layered (overlay) windows.
# usage: powershell -File shot.ps1 x y w h out.png
param([int]$x,[int]$y,[int]$w,[int]$h,[string]$out)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class Cap {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr d,int x,int y,int w,int h,IntPtr s,int sx,int sy,int op);
}
"@
[Cap]::SetProcessDPIAware() | Out-Null
$bmp = New-Object System.Drawing.Bitmap $w,$h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hd = $g.GetHdc(); $sd = [Cap]::GetDC([IntPtr]::Zero)
[Cap]::BitBlt($hd,0,0,$w,$h,$sd,$x,$y,0x40CC0020) | Out-Null
[Cap]::ReleaseDC([IntPtr]::Zero,$sd) | Out-Null; $g.ReleaseHdc($hd)
$bmp.Save($out)

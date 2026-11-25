$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName System.Drawing
$bitmap = [Drawing.Bitmap]::new(256,256)
$g = [Drawing.Graphics]::FromImage($bitmap)
$g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.Clear([Drawing.Color]::Transparent)
$silver = [Drawing.Pen]::new([Drawing.ColorTranslator]::FromHtml('#cabfb1'),10)
$ruby = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#ae1236'))
$dark = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#101012'))
$g.FillEllipse($dark,12,12,232,232)
$g.DrawEllipse($silver,17,17,222,222)
$points = [Drawing.Point[]]@([Drawing.Point]::new(38,128),[Drawing.Point]::new(128,63),[Drawing.Point]::new(218,128),[Drawing.Point]::new(128,193))
$g.DrawPolygon($silver,$points)
$g.FillEllipse($ruby,96,96,64,64)
$g.DrawEllipse($silver,91,91,74,74)
$g.Dispose()
$png = [IO.MemoryStream]::new()
$bitmap.Save($png,[Drawing.Imaging.ImageFormat]::Png)
$bytes=$png.ToArray()
[IO.File]::WriteAllBytes((Join-Path $root 'src/ui/oracle.png'),$bytes)
$ico = [IO.MemoryStream]::new()
$writer = [IO.BinaryWriter]::new($ico)
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]1)
$writer.Write([byte]0); $writer.Write([byte]0); $writer.Write([byte]0); $writer.Write([byte]0)
$writer.Write([uint16]1); $writer.Write([uint16]32); $writer.Write([uint32]$bytes.Length); $writer.Write([uint32]22); $writer.Write($bytes)
[IO.File]::WriteAllBytes((Join-Path $root 'src-tauri/icons/icon.ico'),$ico.ToArray())
$writer.Dispose();$ico.Dispose();$png.Dispose();$bitmap.Dispose();$silver.Dispose();$ruby.Dispose();$dark.Dispose()
# The installer draws the same Oracle identity over the inherited eclipse wallpaper.
$wordmark=[Drawing.Bitmap]::new(900,190)
$g=[Drawing.Graphics]::FromImage($wordmark)
$g.TextRenderingHint=[Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$font=[Drawing.Font]::new('Georgia',102,[Drawing.FontStyle]::Regular,[Drawing.GraphicsUnit]::Pixel)
$brush=[Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#cabfb1'))
$format=[Drawing.StringFormat]::new();$format.Alignment=[Drawing.StringAlignment]::Center;$format.LineAlignment=[Drawing.StringAlignment]::Center
$g.DrawString('O R A C L E',$font,$brush,[Drawing.RectangleF]::new(0,0,900,190),$format)
$wordmark.Save((Join-Path $root 'installer/wordmark.png'),[Drawing.Imaging.ImageFormat]::Png)
$wordmark.Dispose();$g.Dispose();$font.Dispose();$brush.Dispose();$format.Dispose()

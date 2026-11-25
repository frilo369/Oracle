// Captures only the foreground, owned Oracle preview, including its native labels.
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
class OverlayCapture {
    [StructLayout(LayoutKind.Sequential)] struct Rect {public int Left,Top,Right,Bottom;}
    [StructLayout(LayoutKind.Sequential)] struct Point {public int X,Y;}
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h,out Rect r);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h,ref Point p);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h,StringBuilder text,int count);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
    [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h,IntPtr dc);
    [DllImport("gdi32.dll")] static extern bool BitBlt(IntPtr dest,int x,int y,int w,int h,IntPtr source,int sx,int sy,uint operation);
    [DllImport("dwmapi.dll")] static extern int DwmFlush();
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h,int attribute,out int value,int size);
    [DllImport("user32.dll")] static extern bool GetLayeredWindowAttributes(IntPtr h,out uint color,out byte alpha,out uint flags);
    [DllImport("user32.dll")] static extern int GetWindowLong(IntPtr h,int index);
    static int Main(string[] args) {
        if(args[0]=="decode") {using(var image=new Bitmap(args[1]))image.Save(args[2],ImageFormat.Bmp);return 0;}
        SetProcessDpiAwarenessContext(new IntPtr(-4));
        var h=new IntPtr(long.Parse(args[0]));uint pid;GetWindowThreadProcessId(h,out pid);
        var title=new StringBuilder(200);GetWindowText(h,title,200);
        if(pid!=uint.Parse(args[1])||GetForegroundWindow()!=h||!title.ToString().StartsWith("Oracle")||!title.ToString().Contains("overlay preview"))throw new Exception("Only the owned foreground Oracle preview may be captured");
        if(args.Length>3){var overlay=new IntPtr(long.Parse(args[3]));int cloaked;uint color,flags;byte alpha;DwmGetWindowAttribute(overlay,14,out cloaked,4);bool valid=GetLayeredWindowAttributes(overlay,out color,out alpha,out flags);Console.WriteLine("Layered: "+valid+" alpha="+alpha+" key="+color+" flags="+flags+" cloaked="+cloaked+" styles="+GetWindowLong(overlay,-20));}
        System.Threading.Thread.Sleep(150);DwmFlush();
        if(GetForegroundWindow()!=h)throw new Exception("Preview lost foreground before capture");
        Rect r;Point p=new Point();if(!GetClientRect(h,out r)||!ClientToScreen(h,ref p))return 1;
        using(var image=new Bitmap(r.Right,r.Bottom))using(var graphics=Graphics.FromImage(image)) {
            var source=GetDC(IntPtr.Zero);var dest=graphics.GetHdc();
            try{if(!BitBlt(dest,0,0,r.Right,r.Bottom,source,p.X,p.Y,0x40CC0020))throw new Exception("Capture failed");}
            finally{graphics.ReleaseHdc(dest);ReleaseDC(IntPtr.Zero,source);}
            image.Save(args[2],ImageFormat.Png);
            var bits=image.LockBits(new Rectangle(0,0,image.Width,image.Height),ImageLockMode.ReadOnly,PixelFormat.Format32bppArgb);
            try{
                var bytes=new byte[bits.Stride*image.Height];Marshal.Copy(bits.Scan0,bytes,0,bytes.Length);int painted=0;
                for(int i=0;i<bytes.Length;i+=4)if(bytes[i]==26&&bytes[i+1]==16&&bytes[i+2]==15)painted++;
                Console.WriteLine("Native label pixels: "+painted);
                if(painted<1000)throw new Exception("Native labels are absent from the desktop capture");
            }finally{image.UnlockBits(bits);}
        }
        return 0;
    }
}

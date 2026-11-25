// Owned stand-in for borderless League. Accepts test instructions on stdin only.
using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using System.Threading;
class GuideBorderless : Form {
    bool minimizeOnBlur;
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, IntPtr pid);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint from, uint to, bool attach);
    [DllImport("user32.dll")] static extern IntPtr SendMessageTimeout(IntPtr hwnd,uint message,UIntPtr w,IntPtr l,uint flags,uint timeout,out UIntPtr result);
    [DllImport("user32.dll")] static extern bool RegisterHotKey(IntPtr hwnd, int id, uint modifiers, uint key);
    [DllImport("user32.dll")] static extern bool UnregisterHotKey(IntPtr hwnd, int id);
    [DllImport("user32.dll")] static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    public GuideBorderless() {
        Text="Oracle test game"; FormBorderStyle=FormBorderStyle.None;
        WindowState=FormWindowState.Maximized; BackColor=System.Drawing.Color.MidnightBlue;
        Deactivate += delegate { if(minimizeOnBlur) WindowState=FormWindowState.Minimized; };
        Shown += delegate { Console.WriteLine("ready"); new Thread(Read){IsBackground=true}.Start(); };
    }
    void Read() {
        string line;
        while((line=Console.ReadLine())!=null) {
            if(line=="focus") { Invoke((Action)(()=>{
                if(WindowState==FormWindowState.Minimized)WindowState=FormWindowState.Normal;
                Activate();SetForegroundWindow(Handle);
                IntPtr front=GetForegroundWindow();UIntPtr ignored;
                if(front!=Handle && front!=IntPtr.Zero && SendMessageTimeout(front,0,UIntPtr.Zero,IntPtr.Zero,3,50,out ignored)!=IntPtr.Zero) {
                    uint own=GetCurrentThreadId(),other=GetWindowThreadProcessId(front,IntPtr.Zero);
                    if(own!=other && AttachThreadInput(own,other,true)) {try{SetForegroundWindow(Handle);}finally{AttachThreadInput(own,other,false);}}
                }
            })); Thread.Sleep(100); Console.WriteLine("focused:"+(GetForegroundWindow()==Handle)); }
            else if(line=="front") Console.WriteLine("focused:"+(GetForegroundWindow()==Handle));
            else if(line=="fullscreen") { Invoke((Action)(()=>{minimizeOnBlur=true;}));Console.WriteLine("fullscreen:True"); }
            else if(line=="state") { bool minimized=false;Invoke((Action)(()=>minimized=WindowState==FormWindowState.Minimized));Console.WriteLine("minimized:"+minimized); }
            else if(line=="reserve") { bool ok=false;Invoke((Action)(()=>{ok=RegisterHotKey(Handle,731,0x4000,112);}));Console.WriteLine("reserved:"+ok); }
            else if(line=="release") { Invoke((Action)(()=>{UnregisterHotKey(Handle,731);}));Console.WriteLine("released"); }
            else if(line=="f1") {if(GetForegroundWindow()!=Handle){Console.WriteLine("wrong-foreground");continue;}keybd_event(112,0,0,UIntPtr.Zero);Thread.Sleep(50);keybd_event(112,0,2,UIntPtr.Zero);Console.WriteLine("pressed");}
            else if(line=="shift-f1" || line=="ctrl-g") {
                byte modifier=(byte)(line=="shift-f1"?16:17),key=(byte)(line=="shift-f1"?112:71);
                keybd_event(modifier,0,0,UIntPtr.Zero);keybd_event(key,0,0,UIntPtr.Zero);
                Thread.Sleep(50);keybd_event(key,0,2,UIntPtr.Zero);keybd_event(modifier,0,2,UIntPtr.Zero);Console.WriteLine("pressed");
            }
            else if(line=="quit") {BeginInvoke((Action)Close);return;}
        }
    }
    [STAThread] static void Main(){Application.Run(new GuideBorderless());}
}

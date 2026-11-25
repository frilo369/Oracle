// Oracle Setup: one WinForms EXE with its own drawn interface. SetupEngine.cs runs the
// installation directly through .NET and Windows APIs. scripts/build-winforms-setup.ps1
// compiles this file twice with the .NET Framework 4 csc.exe, so it must stay C# 5: Setup.exe,
// and with /define:UNINSTALLER the small Uninstall.exe that the payload carries.
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;
using System.Web.Script.Serialization;

namespace Oracle.Installer
{
    enum SetupMode { Install, Update, Uninstall }

    sealed class Options
    {
        public SetupMode Mode;
        public bool Silent;
        public bool Fixture;
        public int WaitPid = 0;
        public string Root;
        public string Bin = null;
    }

    static class Program
    {
        [STAThread]
        static int Main()
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            if (!Environment.Is64BitOperatingSystem)
            {
                if (!Cli.Has("/S")) MessageBox.Show("Oracle requires 64-bit Windows.", "Oracle", MessageBoxButtons.OK, MessageBoxIcon.Stop);
                return 1;
            }
#if UNINSTALLER
            return RunUninstaller();
#else
            return RunSetup();
#endif
        }

        static string LocalRoot
        {
            get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "oracle"); }
        }

#if UNINSTALLER
        static int RunUninstaller()
        {
            bool silent = Cli.Has("/S");
            string bin = Cli.InstallDir;
            if (bin == null)
            {
                // Run from a temporary copy so this file and its folder can be removed.
                try
                {
                    string self = Application.ExecutablePath;
                    string copy = Path.Combine(Path.GetTempPath(), "Oracle-Uninstall-" + Guid.NewGuid().ToString("N") + ".exe");
                    File.Copy(self, copy);
                    ProcessStartInfo info = new ProcessStartInfo(copy, (Cli.Options + " /UNINSTALLPID=" + Process.GetCurrentProcess().Id + " _?=" + Path.GetDirectoryName(self)).Trim());
                    info.UseShellExecute = false;
                    using (Process.Start(info)) { }
                    return 0;
                }
                catch (Exception ex)
                {
                    if (!silent) MessageBox.Show("The uninstaller could not start: " + ex.Message, "Oracle", MessageBoxButtons.OK, MessageBoxIcon.Stop);
                    return 1;
                }
            }
            int parentPid;
            if (int.TryParse(Cli.Value("/UNINSTALLPID="), out parentPid) && parentPid > 0)
            {
                try { using (Process parent = Process.GetProcessById(parentPid)) if (!parent.WaitForExit(10000)) return 1; }
                catch (ArgumentException) { }
            }
            Options options = new Options();
            options.Mode = SetupMode.Uninstall;
            options.Silent = silent;
            options.Fixture = Cli.Has("/VERIFY-PAYLOAD");
            try { options.Bin = Path.GetFullPath(bin).TrimEnd('\\'); }
            catch (Exception) { return 1; }
            options.Root = Path.GetDirectoryName(options.Bin);
            bool valid = options.Fixture
                ? File.Exists(Path.Combine(options.Bin, ".verify-payload"))
                : string.Equals(options.Bin, Path.Combine(LocalRoot, "bin"), StringComparison.OrdinalIgnoreCase);
            if (!valid)
            {
                if (!silent) MessageBox.Show("This uninstaller is not in the Oracle folder, so nothing was removed.", "Oracle", MessageBoxButtons.OK, MessageBoxIcon.Stop);
                return 1;
            }
            return silent ? RunSilent(options) : RunInteractive(options);
        }
#else
        // The native updater hands this verified installer its PID, then exits.
        static int RunSetup()
        {
            Options options = new Options();
            options.Silent = Cli.Has("/S");
            string fixture = Path.Combine(Path.GetDirectoryName(Application.ExecutablePath), "fixture");
            if (Cli.Has("/VERIFY-PAYLOAD"))
            {
                options.Mode = SetupMode.Install;
                options.Fixture = true;
                options.Root = fixture;
            }
            else if (Cli.Has("/VERIFY-UPDATE") || Cli.Has("/UPDATE"))
            {
                options.Mode = SetupMode.Update;
                options.Fixture = Cli.Has("/VERIFY-UPDATE");
                options.Root = options.Fixture ? fixture : LocalRoot;
                int pid;
                if (!options.Silent || !int.TryParse(Cli.Value("/WAITPID=") ?? "", out pid) || pid <= 0) return 1;
                options.WaitPid = pid;
            }
            else if (options.Silent)
            {
                return 1;
            }
            else
            {
                options.Mode = SetupMode.Install;
                options.Root = LocalRoot;
            }
            return options.Silent ? RunSilent(options) : RunInteractive(options);
        }
#endif

        static int RunInteractive(Options options)
        {
            Session session;
            try { session = new Session(options); }
            catch (Exception ex)
            {
                MessageBox.Show("Setup could not prepare its files: " + ex.Message, "Oracle", MessageBoxButtons.OK, MessageBoxIcon.Stop);
                return 1;
            }
            using (session)
            using (SetupForm form = new SetupForm(options, session))
            {
                Application.Run(form);
                return form.ExitCode;
            }
        }

        // Silent runs show nothing and keep operation output in %TEMP%.
        static int RunSilent(Options options)
        {
            string mode = Session.ModeName(options.Mode);
            string logPath = Path.Combine(Path.GetTempPath(), options.Mode == SetupMode.Uninstall ? "Oracle-Uninstall.log" : "Oracle-Setup.log");
            object gate = new object();
            StreamWriter log = null;
            try
            {
                log = new StreamWriter(logPath, false, new UTF8Encoding(false));
                log.AutoFlush = true;
            }
            catch (Exception) { log = null; }
            Action<string> write = delegate(string line)
            {
                lock (gate)
                {
                    if (log == null) return;
                    try { log.WriteLine(line); } catch (Exception) { }
                }
            };
            int exitCode = 1;
            bool restartUpdate = false;
            try
            {
                if (options.Mode == SetupMode.Update)
                {
                    WaitForUpdateParent(options);
                    restartUpdate = true;
                }
                write("Oracle Setup " + BuildInfo.Version + ": " + mode + (options.Fixture ? " (verification)" : ""));
                using (Session session = new Session(options))
                {
                    exitCode = session.Run(mode, false, write);
                    write("Exit code " + exitCode);
                    return exitCode;
                }
            }
            catch (Exception ex)
            {
                write("##error " + ex.Message);
                return 1;
            }
            finally
            {
                if (restartUpdate) FinishUpdate(options, exitCode, write);
                lock (gate)
                {
                    if (log != null) { log.Dispose(); log = null; }
                }
            }
        }

        static void WaitForUpdateParent(Options options)
        {
            string expected = Path.GetFullPath(Path.Combine(options.Root, @"bin\oracle.exe"));
            Process parent;
            try { parent = Process.GetProcessById(options.WaitPid); }
            catch (ArgumentException) { return; } // Oracle exited before setup initialized.
            using (parent)
            {
                try
                {
                    if (!string.Equals(Path.GetFullPath(parent.MainModule.FileName), expected, StringComparison.OrdinalIgnoreCase))
                        throw new IOException("The update parent is not the installed Oracle.");
                }
                catch (InvalidOperationException) { if (parent.HasExited) return; throw; }
                catch (Win32Exception) { if (parent.HasExited) return; throw; }
                if (!parent.WaitForExit(30000)) throw new IOException("Oracle did not exit for the update; nothing was changed.");
            }
        }

        static void FinishUpdate(Options options, int code, Action<string> write)
        {
            try
            {
                string profile = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "com.frilo369.oracle");
                if (options.Fixture)
                {
                    profile = Environment.GetEnvironmentVariable("ORACLE_DATA_DIR");
                    string allowed = Path.GetFullPath(Path.GetDirectoryName(options.Root)) + Path.DirectorySeparatorChar;
                    if (string.IsNullOrEmpty(profile) || !Path.GetFullPath(profile).StartsWith(allowed, StringComparison.OrdinalIgnoreCase))
                        throw new IOException("The fixture restart needs its disposable profile.");
                }
                SetupEngine.CheckPath(profile); Directory.CreateDirectory(profile);
                string result = new JavaScriptSerializer().Serialize(new { ok = code == 0, version = BuildInfo.Version });
                File.WriteAllText(Path.Combine(profile, "update-result.json"), result, new UTF8Encoding(false));
                string exe = Path.Combine(options.Root, @"bin\oracle.exe");
                if (!File.Exists(exe)) throw new IOException("The app is unavailable after setup. See the setup log before retrying.");
                ProcessStartInfo info = new ProcessStartInfo(exe, (options.Fixture ? "--fixture " : "") + "--tray --skip-update-once");
                info.UseShellExecute = false; info.CreateNoWindow = true;
                using (Process restarted = Process.Start(info))
                {
                    if (options.Fixture) File.WriteAllText(Path.Combine(options.Root, "update-restart.json"), new JavaScriptSerializer().Serialize(new { ok = code == 0, pid = restarted.Id, exe = exe }));
                }
            }
            catch (Exception ex) { write("##error Could not restart Oracle: " + ex.Message); }
        }
    }

    // The command line as NSIS reads it: switches are words, and _?=<folder> takes the rest.
    static class Cli
    {
        static string raw;

        public static string Raw
        {
            get
            {
                if (raw != null) return raw;
                string line = Environment.CommandLine.TrimStart();
                int end;
                if (line.StartsWith("\""))
                {
                    end = line.IndexOf('"', 1);
                    end = end < 0 ? line.Length : end + 1;
                }
                else
                {
                    end = line.IndexOf(' ');
                    if (end < 0) end = line.Length;
                }
                raw = line.Substring(end).Trim();
                return raw;
            }
        }

        public static string InstallDir
        {
            get
            {
                int at = Raw.IndexOf("_?=", StringComparison.Ordinal);
                if (at < 0) return null;
                string dir = Raw.Substring(at + 3).Trim().Trim('"').TrimEnd('\\');
                return dir.Length == 0 ? null : dir;
            }
        }

        public static string Options
        {
            get
            {
                int at = Raw.IndexOf("_?=", StringComparison.Ordinal);
                return (at < 0 ? Raw : Raw.Substring(0, at)).Trim();
            }
        }

        static string[] Words
        {
            get { return Options.Split(new char[] { ' ', '\t' }, StringSplitOptions.RemoveEmptyEntries); }
        }

        public static bool Has(string flag)
        {
            foreach (string word in Words)
                if (string.Equals(word.Trim('"'), flag, StringComparison.OrdinalIgnoreCase)) return true;
            return false;
        }

        public static string Value(string prefix)
        {
            foreach (string word in Words)
            {
                string w = word.Trim('"');
                if (w.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)) return w.Substring(prefix.Length);
            }
            return null;
        }
    }

    static class Resources
    {
        public static Stream Open(string name)
        {
            Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(name);
            if (stream == null) throw new FileNotFoundException("Setup is missing " + name + ".");
            return stream;
        }

        public static Bitmap Image(string name)
        {
            using (Stream stream = Open(name))
            using (Bitmap decoded = new Bitmap(stream))
                return new Bitmap(decoded);
        }

        public static string Text(string name)
        {
            using (StreamReader reader = new StreamReader(Open(name), Encoding.UTF8))
                return reader.ReadToEnd();
        }
    }

    // Private scratch files; installation itself stays inside this compiled process.
    sealed class Session : IDisposable
    {
        readonly Options options;
        readonly string work;

        public Session(Options options)
        {
            this.options = options;
            work = SetupEngine.PrivateDirectory(Path.GetTempPath(), "oracle-setup-");
        }

        public static string ModeName(SetupMode mode)
        {
            switch (mode)
            {
                case SetupMode.Update: return "Update";
                case SetupMode.Uninstall: return "Uninstall";
                default: return "Install";
            }
        }

        // Called on the UI's worker thread; the existing progress protocol stays unchanged.
        public int Run(string mode, bool acceptLtk, Action<string> onLine)
        {
            return new SetupEngine(options, work, onLine).Run(mode, acceptLtk);
        }

        public void Dispose()
        {
            try { SetupEngine.RemoveWork(work); } catch (Exception) { }
        }
    }

    static class Native
    {
        [DllImport("dwmapi.dll")]
        public static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

        [DllImport("uxtheme.dll", CharSet = CharSet.Unicode)]
        public static extern int SetWindowTheme(IntPtr hwnd, string appName, string idList);

        [DllImport("user32.dll")]
        public static extern bool SystemParametersInfo(int action, int param, ref bool value, int winIni);

        public static bool ClientAreaAnimation()
        {
            bool enabled = true;
            try
            {
                if (!SystemParametersInfo(0x1042, 0, ref enabled, 0)) return true;
            }
            catch (Exception) { return true; }
            return enabled;
        }
    }

    sealed class SetupForm : Form
    {
        enum Phase { Probing, Ready, Running, Done, Failed }
        enum Hit { None, Minimise, Close, Primary, Secondary, Accept, Licence }

        const string LicenceUrl = "https://developer.riotgames.com/docs/lol";
        const string AcceptText = "Oracle League companion";
        const string LicenceText = "View licence";

        Color Silver = Color.FromArgb(0xCA, 0xBF, 0xB1);
        Color SteelDeep = Color.FromArgb(0xA7, 0x9D, 0x92);
        Color SteelDark = Color.FromArgb(0x53, 0x50, 0x54);
        Color Cream = Color.FromArgb(0xFF, 0xF1, 0xDB);
        Color CreamSoft = Color.FromArgb(0xEC, 0xDF, 0xC6);
        static readonly Color Charcoal = Color.FromArgb(0x10, 0x10, 0x12);
        Color Ruby = Color.FromArgb(0xD0, 0x35, 0x59);
        static readonly Color ErrorRed = Color.FromArgb(0xFF, 0x7A, 0x6B);
        static readonly Color Success = Color.FromArgb(0x6E, 0xE7, 0xA0);
        static readonly Color LogBack = Color.FromArgb(0x0D, 0x0C, 0x0E);
        static readonly Color LogDim = Color.FromArgb(0xB0, 0xA6, 0x94);

        readonly Options options;
        readonly Session session;
        readonly float scale;
        readonly bool motion;
        readonly Random random = new Random();
        readonly Stopwatch clock = Stopwatch.StartNew();
        readonly System.Windows.Forms.Timer timer = new System.Windows.Forms.Timer();
        readonly Dictionary<string, string> probe = new Dictionary<string, string>();
        readonly RichTextBox log = new RichTextBox();

        Bitmap wallpaper, logo, logoRed, logoRuby, background;
        TextureBrush scanlines;
        Font headingFont, statusFont, smallFont, buttonFont, linkFont;

        Rectangle logoRect, pillRect, versionRect, minRect, closeRect, panelRect, headingRect, statusRect;
        Rectangle logFrame, progressRect, footerRect, dotRect, footerTextRect, primaryRect, secondaryRect;
        Rectangle boxRect, acceptRect, acceptTextRect, licenceRect;

        Phase phase = Phase.Probing;
        bool scriptRunning, blockClose, probeFailed, ltkNeeded, accepted, showLicence, progressVisible;
        string heading = "", status = "", footer = "", primaryText = "", secondaryText = "", lastError;
        bool primaryEnabled, primaryVisible = true, secondaryEnabled = true, secondaryVisible = true;
        Color statusColor, dotColor;
        float progressShown;
        int progressTarget, logLines;
        Hit hover = Hit.None, pressed = Hit.None;
        Rectangle beamRect = Rectangle.Empty;
        bool glitching;
        long glitchEnd, nextGlitch, nextJitter;
        int glitchShift;
        Rectangle[] slices = new Rectangle[0];
        int[] sliceShift = new int[0];
        Rectangle[] noise = new Rectangle[0];

        public int ExitCode = 2;

        public SetupForm(Options options, Session session)
        {
            this.options = options;
            this.session = session;
            string wallpaperName = ReadAppearance(options.Fixture ? options.Root : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "com.frilo369.oracle"));
            statusColor = CreamSoft;
            dotColor = Silver;
            using (Graphics screen = Graphics.FromHwnd(IntPtr.Zero)) scale = screen.DpiX / 96f;
            motion = Native.ClientAreaAnimation();

            Text = options.Mode == SetupMode.Uninstall ? "Uninstall Oracle" : "Oracle Setup";
            FormBorderStyle = FormBorderStyle.None;
            MaximizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;
            AutoScaleMode = AutoScaleMode.None;
            ClientSize = new Size(S(880), S(495));
            BackColor = Charcoal;
            KeyPreview = true;
            DoubleBuffered = true;
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint, true);
            try { using (Stream icon = Resources.Open("Oracle.icon.ico")) Icon = new Icon(icon); } catch (Exception) { }

            headingFont = new Font("Segoe UI Semibold", 22f * scale, FontStyle.Regular, GraphicsUnit.Pixel);
            statusFont = new Font("Segoe UI", 13f * scale, FontStyle.Regular, GraphicsUnit.Pixel);
            linkFont = new Font("Segoe UI", 13f * scale, FontStyle.Underline, GraphicsUnit.Pixel);
            smallFont = new Font("Segoe UI", 12f * scale, FontStyle.Regular, GraphicsUnit.Pixel);
            buttonFont = new Font("Segoe UI Semibold", 13f * scale, FontStyle.Regular, GraphicsUnit.Pixel);

            logoRect = R(290, 22, 300, 110);
            pillRect = R(728, 20, 128, 30);
            versionRect = R(728, 20, 60, 30);
            minRect = R(788, 20, 34, 30);
            closeRect = R(822, 20, 34, 30);
            panelRect = R(46, 150, 788, 254);
            headingRect = R(70, 162, 740, 34);
            statusRect = R(70, 198, 740, 22);
            logFrame = R(70, 228, 740, 120);
            progressRect = R(70, 372, 740, 6);
            footerRect = R(24, 428, 832, 46);
            dotRect = R(42, 447, 9, 9);
            footerTextRect = R(60, 440, 480, 22);
            primaryRect = R(717, 435, 132, 32);
            secondaryRect = R(575, 435, 132, 32);
            boxRect = R(70, 368, 16, 16);
            Size acceptSize = TextRenderer.MeasureText(AcceptText, statusFont);
            Size licenceSize = TextRenderer.MeasureText(LicenceText, statusFont);
            acceptTextRect = new Rectangle(boxRect.Right + S(10), boxRect.Y - S(3), acceptSize.Width, boxRect.Height + S(6));
            acceptRect = Rectangle.Union(boxRect, acceptTextRect);
            licenceRect = new Rectangle(acceptRect.Right + S(18), acceptTextRect.Y, licenceSize.Width, acceptTextRect.Height);

            wallpaper = Resources.Image(wallpaperName);
            using (Bitmap source = Resources.Image("Oracle.logo.png"))
            {
                float k = Math.Min(logoRect.Width / (float)source.Width, logoRect.Height / (float)source.Height);
                Size size = new Size((int)Math.Round(source.Width * k), (int)Math.Round(source.Height * k));
                logoRect = new Rectangle(logoRect.X + (logoRect.Width - size.Width) / 2, logoRect.Y + (logoRect.Height - size.Height) / 2, size.Width, size.Height);
                logo = Scaled(source, size, wallpaperName == "Oracle.wallpaper.jpg" ? null : JewelMatrix());
                // Glint layers use the selected jewel and silver highlights.
                logoRed = Scaled(logo, size, Tint(Ruby.R / 220f, Ruby.G / 220f, Ruby.B / 220f, .55f));
                logoRuby = Scaled(logo, size, Tint(1.15f, .98f, 1.0f, .55f));
            }
            int period = Math.Max(3, (int)Math.Round(3 * scale));
            int dark = Math.Max(1, (int)Math.Round(scale));
            using (Bitmap tile = new Bitmap(1, period, PixelFormat.Format32bppArgb))
            {
                for (int y = 0; y < period; y++) tile.SetPixel(0, y, y >= period - dark ? Color.FromArgb(34, 0, 0, 0) : Color.Transparent);
                scanlines = new TextureBrush(tile, WrapMode.Tile);
            }

            log.BorderStyle = BorderStyle.None;
            log.ReadOnly = true;
            log.BackColor = LogBack;
            log.ForeColor = LogDim;
            log.Font = new Font("Consolas", 9f);
            log.ScrollBars = RichTextBoxScrollBars.Vertical;
            log.DetectUrls = true;
            log.TabStop = false;
            log.WordWrap = true;
            log.Bounds = R(82, 236, 722, 104);
            log.HandleCreated += delegate { Native.SetWindowTheme(log.Handle, "DarkMode_Explorer", null); };
            log.LinkClicked += delegate(object sender, LinkClickedEventArgs e) { OpenUrl(e.LinkText); };
            Controls.Add(log);

            timer.Interval = 33;
            timer.Tick += Tick;
            nextGlitch = 2200 + random.Next(0, 3000);
        }

        int S(float value) { return (int)Math.Round(value * scale); }

        Rectangle R(float x, float y, float w, float h)
        {
            return Rectangle.FromLTRB(S(x), S(y), S(x + w), S(y + h));
        }

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams p = base.CreateParams;
                p.Style |= 0x20000;       // WS_MINIMIZEBOX: the taskbar button can minimise it
                p.ClassStyle |= 0x20000;  // CS_DROPSHADOW
                return p;
            }
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            try
            {
                int round = 2;            // DWMWCP_ROUND
                Native.DwmSetWindowAttribute(Handle, 33, ref round, 4);
                int border = ColorTranslator.ToWin32(SteelDeep);
                Native.DwmSetWindowAttribute(Handle, 34, ref border, 4);
            }
            catch (Exception) { }
        }

        protected override void OnShown(EventArgs e)
        {
            base.OnShown(e);
            if (options.Mode == SetupMode.Uninstall) ShowUninstallReady();
            else StartProbe();
            UpdateTimer();
        }

        protected override void OnResize(EventArgs e) { base.OnResize(e); UpdateTimer(); }
        protected override void OnVisibleChanged(EventArgs e) { base.OnVisibleChanged(e); UpdateTimer(); }

        void UpdateTimer()
        {
            bool on = motion && Visible && WindowState != FormWindowState.Minimized;
            if (timer.Enabled != on) timer.Enabled = on;
        }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            if (blockClose && e.CloseReason == CloseReason.UserClosing) e.Cancel = true;
            base.OnFormClosing(e);
        }

        protected override void OnFormClosed(FormClosedEventArgs e)
        {
            timer.Stop();
            base.OnFormClosed(e);
        }

        // WinForms disposes a closed form while its message loop still runs, and destroying the
        // log control can still paint this form: free the drawing resources last, then stop
        // painting with them.
        protected override void Dispose(bool disposing)
        {
            if (disposing) timer.Stop();
            base.Dispose(disposing);
            if (!disposing) return;
            timer.Dispose();
            Bitmap[] bitmaps = { wallpaper, logo, logoRed, logoRuby, background };
            wallpaper = logo = logoRed = logoRuby = background = null;
            foreach (Bitmap b in bitmaps) if (b != null) b.Dispose();
            Font[] fonts = { headingFont, statusFont, smallFont, buttonFont, linkFont };
            foreach (Font f in fonts) if (f != null) f.Dispose();
            if (scanlines != null) scanlines.Dispose();
            scanlines = null;
        }

        // ----- States -----

        void StartProbe()
        {
            phase = Phase.Probing;
            probeFailed = false;
            probe.Clear();
            heading = "Install Oracle";
            SetStatus("Checking this PC\u2026", CreamSoft);
            footer = "Checking";
            dotColor = Ruby;
            primaryText = "Install";
            primaryEnabled = false;
            primaryVisible = true;
            secondaryText = "Cancel";
            secondaryEnabled = true;
            secondaryVisible = true;
            showLicence = false;
            progressVisible = false;
            RunScript("Probe");
        }

        string ProbeValue(string key, string fallback)
        {
            string value;
            return probe.TryGetValue(key, out value) ? value : fallback;
        }

        void ShowReady()
        {
            phase = Phase.Ready;
            string installed = ProbeValue("installed", "none");
            bool running = ProbeValue("oracle", "stopped") == "running";
            ltkNeeded = false;
            bool webviewMissing = ProbeValue("webview", "ok") == "missing";
            Version current = new Version(BuildInfo.Version);
            Version found = null;
            if (installed != "none")
            {
                try { found = new Version(installed); } catch (Exception) { found = null; }
            }
            if (found == null)
            {
                heading = "Install Oracle";
                primaryText = "Install";
                SetStatus("Installs for this Windows account. Your League guide, one shortcut away.", CreamSoft);
            }
            else if (found < current)
            {
                heading = "Update Oracle";
                primaryText = "Update";
                SetStatus("Updates Oracle " + installed + " to " + BuildInfo.Version + ". Your preferences stay in place.", CreamSoft);
            }
            else if (found == current)
            {
                heading = "Reinstall Oracle";
                primaryText = "Reinstall";
                SetStatus("Oracle " + installed + " is installed. Reinstalling keeps your preferences.", CreamSoft);
            }
            else
            {
                heading = "Install Oracle " + BuildInfo.Version;
                primaryText = "Install";
                SetStatus("Oracle " + installed + " is installed. Installing " + BuildInfo.Version + " replaces it.", Silver);
            }
            if (running) SetStatus("Oracle is running. Setup closes it first. Your preferences stay in place.", Silver);
            string bin = Path.Combine(options.Root, "bin");
            if (options.Fixture) AppendLog("Verification install into " + bin + ". Nothing outside that folder changes.", LogDim);
            else AppendLog("Oracle " + BuildInfo.Version + " installs to " + bin + ".", LogDim);
            if (webviewMissing) AppendLog("Setup also installs Microsoft Edge WebView2 Runtime, which Oracle needs.", LogDim);
            if (ltkNeeded) ShowLicenceTerms();
            footer = options.Fixture ? "Ready \u00B7 verification copy" : "Ready";
            dotColor = Silver;
            primaryEnabled = !ltkNeeded || accepted;
            secondaryText = "Cancel";
            secondaryEnabled = true;
            secondaryVisible = true;
            Invalidate();
        }

        void ShowLicenceTerms()
        {
#if !UNINSTALLER
            showLicence = true;
            accepted = false;
            AppendLog("", LogDim);
            foreach (string line in new string[] { "Oracle runs in your Windows system tray." })
                AppendLog(line, line.StartsWith("http") ? Silver : CreamSoft);
#endif
        }

        void ShowUninstallReady()
        {
            phase = Phase.Ready;
            heading = "Remove Oracle";
            SetStatus("Your preferences stay on this PC.", CreamSoft);
            AppendLog("Removes the Oracle program from " + options.Bin + (options.Fixture ? " (verification install)." : "."), LogDim);
            AppendLog("Your Oracle preferences stay on this PC.", LogDim);
            footer = "Ready";
            dotColor = Silver;
            primaryText = "Remove";
            primaryEnabled = true;
            secondaryText = "Cancel";
            Invalidate();
        }

        void StartRun()
        {
            bool removing = options.Mode == SetupMode.Uninstall;
            phase = Phase.Running;
            heading = removing ? "Removing Oracle" : primaryText == "Update" ? "Updating Oracle" : "Installing Oracle";
            SetStatus("Starting\u2026", CreamSoft);
            footer = removing ? "Removing" : "Installing";
            dotColor = Ruby;
            showLicence = false;
            progressVisible = true;
            progressTarget = 0;
            progressShown = 0;
            primaryEnabled = false;
            secondaryEnabled = false;
            if (logLines > 0) AppendLog("", LogDim);
            RunScript(removing ? "Uninstall" : "Install");
        }

        void ShowDone()
        {
            phase = Phase.Done;
            ExitCode = 0;
            progressTarget = 100;
            if (!motion) progressShown = 100;
            dotColor = Success;
            primaryEnabled = true;
            primaryVisible = true;
            if (options.Mode == SetupMode.Uninstall)
            {
                heading = "Oracle is removed";
                SetStatus("Your preferences are preserved on this PC.", CreamSoft);
                footer = "Removed";
                primaryText = "Close";
                secondaryVisible = false;
            }
            else if (options.Fixture)
            {
                heading = "Verification install finished";
                SetStatus("The files are in " + Path.Combine(options.Root, "bin") + ".", CreamSoft);
                footer = "Installed \u00B7 verification copy";
                primaryText = "Close";
                secondaryVisible = false;
            }
            else
            {
                heading = "Oracle is installed";
                SetStatus("Oracle " + BuildInfo.Version + " is ready. It is also in your Start menu.", CreamSoft);
                footer = "Installed";
                primaryText = "Launch Oracle";
                secondaryText = "Close";
                secondaryEnabled = true;
                secondaryVisible = true;
            }
        }

        void ShowFailed(int code, bool probing)
        {
            bool removing = options.Mode == SetupMode.Uninstall;
            phase = Phase.Failed;
            probeFailed = probing;
            ExitCode = 1;
            heading = probing ? "Setup could not check this PC" : removing ? "Oracle was not removed" : "Oracle was not installed";
            SetStatus(lastError ?? Fallback(code), ErrorRed);
            footer = probing ? "Stopped" : removing ? "Not removed" : "Not installed";
            dotColor = ErrorRed;
            primaryText = "Retry";
            primaryEnabled = true;
            primaryVisible = true;
            secondaryText = "Close";
            secondaryEnabled = true;
            secondaryVisible = true;
            if (code == 16)
            {
                ltkNeeded = true;
                ShowLicenceTerms();
                primaryEnabled = accepted;
            }
        }

        string Fallback(int code)
        {
            bool removing = options.Mode == SetupMode.Uninstall;
            switch (code)
            {
                case -1: return "Setup could not complete this operation. Close Oracle and choose Retry.";
                case 10: return removing ? "Close Oracle and its diagnostics, then run the uninstaller again." : "Setup could not close Oracle. Quit it from its tray icon, then choose Retry.";
                case 12: return "A League game is running. Finish the game, then choose Retry.";
                case 15: return "WebView2 Runtime installation failed. Check your Internet connection, then choose Retry.";
                case 17: return "Could not check whether League is running.";
                default: return removing ? "Close Oracle and its diagnostics, then run the uninstaller again." : "Setup could not write Oracle. Close Oracle and choose Retry.";
            }
        }

        void SetStatus(string text, Color color)
        {
            status = text;
            statusColor = color;
            Invalidate(statusRect);
        }

        // ----- Installation worker -----

        void RunScript(string mode)
        {
            scriptRunning = true;
            blockClose = mode != "Probe";
            lastError = null;
            bool accept = accepted;
            Thread worker = new Thread(delegate()
            {
                int code;
                try { code = session.Run(mode, accept, Post); }
                catch (Exception ex)
                {
                    Post("##stderr " + ex.Message);
                    code = -1;
                }
                try { BeginInvoke(new Action<string, int>(ScriptExited), mode, code); }
                catch (InvalidOperationException) { }
            });
            worker.IsBackground = true;
            worker.Start();
            Invalidate();
        }

        void Post(string line)
        {
            try
            {
                if (!IsDisposed && IsHandleCreated) BeginInvoke(new Action<string>(OnLine), line);
            }
            catch (InvalidOperationException) { }
        }

        void OnLine(string line)
        {
            if (line.StartsWith("##progress "))
            {
                int value;
                if (int.TryParse(line.Substring(11).Trim(), out value))
                {
                    progressTarget = Math.Max(0, Math.Min(100, value));
                    if (!motion) progressShown = progressTarget;
                    InvalidateProgress();
                    Invalidate(footerTextRect);
                }
            }
            else if (line.StartsWith("##status "))
            {
                SetStatus(line.Substring(9), CreamSoft);
                AppendLog(line.Substring(9), Cream);
            }
            else if (line.StartsWith("##warn "))
            {
                AppendLog(line.Substring(7), Silver);
            }
            else if (line.StartsWith("##error "))
            {
                lastError = line.Substring(8);
                AppendLog(lastError, ErrorRed);
            }
            else if (line.StartsWith("##probe "))
            {
                string pair = line.Substring(8);
                int eq = pair.IndexOf('=');
                if (eq > 0) probe[pair.Substring(0, eq).Trim()] = pair.Substring(eq + 1).Trim();
            }
            else if (line.StartsWith("##stderr "))
            {
                AppendLog(line.Substring(9), Color.FromArgb(0xD0, 0x6A, 0x5E));
            }
            else
            {
                AppendLog(line, LogDim);
            }
        }

        void ScriptExited(string mode, int code)
        {
            scriptRunning = false;
            blockClose = false;
            if (mode == "Probe")
            {
                if (code == 0) ShowReady(); else ShowFailed(code, true);
            }
            else if (code == 0) ShowDone();
            else ShowFailed(code, false);
            Invalidate();
        }

        void AppendLog(string text, Color color)
        {
            if (logLines++ > 0) text = "\n" + text;
            log.SelectionStart = log.TextLength;
            log.SelectionLength = 0;
            log.SelectionColor = color;
            log.AppendText(text);
            log.SelectionStart = log.TextLength;
            log.ScrollToCaret();
        }

        // ----- Input -----

        void Primary()
        {
            if (scriptRunning) return;
            switch (phase)
            {
                case Phase.Ready:
                    StartRun();
                    break;
                case Phase.Failed:
                    if (probeFailed) StartProbe(); else StartRun();
                    break;
                case Phase.Done:
                    if (options.Mode == SetupMode.Install && !options.Fixture) Launch(); else Close();
                    break;
            }
        }

        void Launch()
        {
            string exe = Path.Combine(Path.Combine(options.Root, "bin"), "oracle.exe");
            try
            {
                ProcessStartInfo info = new ProcessStartInfo(exe);
                info.UseShellExecute = true;
                info.WorkingDirectory = Path.GetDirectoryName(exe);
                using (Process.Start(info)) { }
                Close();
            }
            catch (Exception ex)
            {
                SetStatus("Windows could not start Oracle: " + ex.Message, ErrorRed);
            }
        }

        static void OpenUrl(string url)
        {
            if (!url.StartsWith("https://", StringComparison.OrdinalIgnoreCase)) return;
            try
            {
                ProcessStartInfo info = new ProcessStartInfo(url);
                info.UseShellExecute = true;
                using (Process.Start(info)) { }
            }
            catch (Exception) { }
        }

        void TryClose()
        {
            if (!blockClose) Close();
        }

        void Activate(Hit hit)
        {
            switch (hit)
            {
                case Hit.Minimise:
                    WindowState = FormWindowState.Minimized;
                    break;
                case Hit.Close:
                case Hit.Secondary:
                    TryClose();
                    break;
                case Hit.Primary:
                    Primary();
                    break;
                case Hit.Accept:
                    accepted = !accepted;
                    primaryEnabled = accepted;
                    Invalidate();
                    break;
                case Hit.Licence:
                    OpenUrl(LicenceUrl);
                    break;
            }
        }

        Hit HitAt(Point p)
        {
            if (minRect.Contains(p)) return Hit.Minimise;
            if (closeRect.Contains(p)) return Hit.Close;
            if (primaryVisible && primaryEnabled && primaryRect.Contains(p)) return Hit.Primary;
            if (secondaryVisible && secondaryEnabled && secondaryRect.Contains(p)) return Hit.Secondary;
            if (showLicence && !scriptRunning && acceptRect.Contains(p)) return Hit.Accept;
            if (showLicence && licenceRect.Contains(p)) return Hit.Licence;
            return Hit.None;
        }

        Rectangle RectOf(Hit hit)
        {
            switch (hit)
            {
                case Hit.Minimise: return minRect;
                case Hit.Close: return closeRect;
                case Hit.Primary: return primaryRect;
                case Hit.Secondary: return secondaryRect;
                case Hit.Accept: return acceptRect;
                case Hit.Licence: return licenceRect;
                default: return Rectangle.Empty;
            }
        }

        void InvalidateHit(Hit hit)
        {
            Rectangle r = RectOf(hit);
            if (r.IsEmpty) return;
            r.Inflate(S(3), S(3));
            Invalidate(r);
        }

        void SetHover(Hit hit)
        {
            if (hit == hover) return;
            InvalidateHit(hover);
            hover = hit;
            InvalidateHit(hover);
            Cursor = hit == Hit.None ? Cursors.Default : Cursors.Hand;
        }

        protected override void OnMouseMove(MouseEventArgs e) { base.OnMouseMove(e); SetHover(HitAt(e.Location)); }
        protected override void OnMouseLeave(EventArgs e) { base.OnMouseLeave(e); SetHover(Hit.None); }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            if (e.Button != MouseButtons.Left) return;
            pressed = HitAt(e.Location);
            InvalidateHit(pressed);
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            base.OnMouseUp(e);
            Hit was = pressed;
            pressed = Hit.None;
            InvalidateHit(was);
            if (e.Button == MouseButtons.Left && was != Hit.None && HitAt(e.Location) == was) Activate(was);
        }

        protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
        {
            if (keyData == Keys.Enter)
            {
                if (primaryVisible && primaryEnabled) Primary();
                return true;
            }
            if (keyData == Keys.Escape)
            {
                TryClose();
                return true;
            }
            return base.ProcessCmdKey(ref msg, keyData);
        }

        protected override void WndProc(ref Message m)
        {
            const int WM_NCHITTEST = 0x84, WM_NCLBUTTONDBLCLK = 0xA3, HTCLIENT = 1, HTCAPTION = 2;
            if (m.Msg == WM_NCLBUTTONDBLCLK) return;
            base.WndProc(ref m);
            if (m.Msg == WM_NCHITTEST && m.Result == (IntPtr)HTCLIENT)
            {
                long lp = m.LParam.ToInt64();
                Point screen = new Point((short)(lp & 0xFFFF), (short)((lp >> 16) & 0xFFFF));
                if (HitAt(PointToClient(screen)) == Hit.None) m.Result = (IntPtr)HTCAPTION;
            }
        }

        // ----- Motion -----

        Rectangle BeamAt(long now)
        {
            if (!motion) return Rectangle.Empty;
            const long cycle = 7000, sweep = 2800;
            long t = now % cycle;
            if (t >= sweep) return Rectangle.Empty;
            int band = S(90);
            int top = (int)(-band + (ClientSize.Height + band) * (t / (float)sweep));
            return new Rectangle(0, top, ClientSize.Width, band);
        }

        Rectangle GlitchArea
        {
            get
            {
                Rectangle r = logoRect;
                r.Inflate(S(12), S(3));
                return r;
            }
        }

        void Shuffle()
        {
            glitchShift = Math.Max(1, S(3));
            if (random.Next(2) == 0) glitchShift = -glitchShift;
            int count = 1 + random.Next(3);
            slices = new Rectangle[count];
            sliceShift = new int[count];
            for (int i = 0; i < count; i++)
            {
                int h = Math.Max(1, S(3 + random.Next(10)));
                int y = logoRect.Y + random.Next(Math.Max(1, logoRect.Height - h));
                slices[i] = new Rectangle(logoRect.X - S(8), y, logoRect.Width + S(16), h);
                int dx = Math.Max(1, S(2 + random.Next(5)));
                sliceShift[i] = random.Next(2) == 0 ? dx : -dx;
            }
            int lines = 1 + random.Next(2);
            noise = new Rectangle[lines];
            for (int i = 0; i < lines; i++)
            {
                int w = S(120 + random.Next(280));
                int x = random.Next(Math.Max(1, ClientSize.Width - w));
                int y = random.Next(Math.Max(1, S(150)));
                noise[i] = new Rectangle(x, y, w, Math.Max(1, S(1 + random.Next(2))));
            }
        }

        void InvalidateGlitch()
        {
            Invalidate(GlitchArea);
            foreach (Rectangle n in noise) Invalidate(n);
        }

        void InvalidateProgress()
        {
            Rectangle r = progressRect;
            r.Inflate(S(4), S(4));
            Invalidate(r);
        }

        void Tick(object sender, EventArgs e)
        {
            long now = clock.ElapsedMilliseconds;
            Rectangle beam = BeamAt(now);
            if (beam != beamRect)
            {
                if (!beamRect.IsEmpty) Invalidate(beamRect);
                beamRect = beam;
                if (!beamRect.IsEmpty) Invalidate(beamRect);
            }
            if (glitching)
            {
                if (now >= glitchEnd)
                {
                    InvalidateGlitch();
                    glitching = false;
                    nextGlitch = now + random.Next(3500, 8001);
                }
                else if (now >= nextJitter)
                {
                    InvalidateGlitch();
                    Shuffle();
                    InvalidateGlitch();
                    nextJitter = now + 60;
                }
            }
            else if (now >= nextGlitch)
            {
                glitching = true;
                glitchEnd = now + random.Next(120, 221);
                nextJitter = now + 60;
                Shuffle();
                InvalidateGlitch();
            }
            if (Math.Abs(progressShown - progressTarget) > 0.05f)
            {
                progressShown += (progressTarget - progressShown) * 0.18f;
                if (Math.Abs(progressShown - progressTarget) < 0.3f) progressShown = progressTarget;
                InvalidateProgress();
            }
            else if (phase == Phase.Running && progressVisible)
            {
                InvalidateProgress();
            }
            if (phase == Phase.Probing || phase == Phase.Running)
            {
                Rectangle r = dotRect;
                r.Inflate(S(6), S(6));
                Invalidate(r);
            }
        }

        // ----- Painting -----

        static GraphicsPath Round(RectangleF r, float radius)
        {
            GraphicsPath path = new GraphicsPath();
            float d = Math.Min(radius * 2, Math.Min(r.Width, r.Height));
            if (d <= 0.5f)
            {
                path.AddRectangle(r);
                return path;
            }
            path.AddArc(r.X, r.Y, d, d, 180, 90);
            path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
            path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
            path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
            path.CloseFigure();
            return path;
        }

        static RectangleF Edge(Rectangle r)
        {
            return new RectangleF(r.X, r.Y, r.Width - 1, r.Height - 1);
        }

        static Color Mix(Color a, Color b, float t)
        {
            return Color.FromArgb(
                (int)(a.R + (b.R - a.R) * t),
                (int)(a.G + (b.G - a.G) * t),
                (int)(a.B + (b.B - a.B) * t));
        }

        string ReadAppearance(string root)
        {
            const string fallback = "Oracle.wallpaper.jpg";
            try
            {
                string path = Path.Combine(root, "settings.json");
                if (!File.Exists(path)) return fallback;
                string json;
                using (var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
                {
                    if (file.Length > 65536) return fallback;
                    using (var reader = new StreamReader(file)) json = reader.ReadToEnd();
                }
                var preferences = new JavaScriptSerializer { MaxJsonLength = 65536 }.Deserialize<Dictionary<string, object>>(json);
                object selected;
                if (preferences == null || !preferences.TryGetValue("assetColour", out selected)) return fallback;
                string colour = selected as string;
                if (colour != null && System.Text.RegularExpressions.Regex.IsMatch(colour, "^#[0-9a-fA-F]{6}$") && !colour.Equals("#ae1236", StringComparison.OrdinalIgnoreCase))
                {
                    Ruby = ColorTranslator.FromHtml(colour);
                    Silver = Mix(Ruby, Color.White, .60f);
                    Cream = Mix(Ruby, Color.White, .92f);
                    CreamSoft = Mix(Cream, Silver, .25f);
                    SteelDeep = Mix(Silver, Charcoal, .33f);
                    SteelDark = Mix(Silver, Charcoal, .65f);
                }
            }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
            catch (ArgumentException) { }
            catch (InvalidOperationException) { }
            return fallback;
        }

        ColorMatrix JewelMatrix()
        {
            Color metal = Mix(Silver, Cream, .5f);
            return new ColorMatrix(new float[][] {
                new float[] { Ruby.R/255f, Ruby.G/255f, Ruby.B/255f, 0, 0 },
                new float[] { (metal.R-Ruby.R)/255f, (metal.G-Ruby.G)/255f, (metal.B-Ruby.B)/255f, 0, 0 },
                new float[] { 0, 0, 0, 0, 0 }, new float[] { 0, 0, 0, 1, 0 }, new float[] { 0, 0, 0, 0, 1 }
            });
        }

        static ColorMatrix Tint(float r, float g, float b, float alpha)
        {
            float[] luminance = { 0.30f, 0.59f, 0.11f };
            float[][] m = new float[5][];
            for (int i = 0; i < 3; i++) m[i] = new float[] { luminance[i] * r, luminance[i] * g, luminance[i] * b, 0, 0 };
            m[3] = new float[] { 0, 0, 0, alpha, 0 };
            m[4] = new float[] { 0, 0, 0, 0, 1 };
            return new ColorMatrix(m);
        }

        static Bitmap Scaled(Bitmap source, Size size, ColorMatrix matrix)
        {
            Bitmap result = new Bitmap(Math.Max(1, size.Width), Math.Max(1, size.Height), PixelFormat.Format32bppPArgb);
            using (Graphics g = Graphics.FromImage(result))
            using (ImageAttributes attributes = new ImageAttributes())
            {
                g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                g.PixelOffsetMode = PixelOffsetMode.HighQuality;
                g.CompositingQuality = CompositingQuality.HighQuality;
                attributes.SetWrapMode(WrapMode.TileFlipXY);
                if (matrix != null) attributes.SetColorMatrix(matrix);
                g.DrawImage(source, new Rectangle(Point.Empty, size), 0, 0, source.Width, source.Height, GraphicsUnit.Pixel, attributes);
            }
            return result;
        }

        void FillGlass(Graphics g, Rectangle rect, float radius, Color fill, int edgeTop, int edgeBottom)
        {
            using (GraphicsPath path = Round(rect, S(radius)))
            {
                using (SolidBrush b = new SolidBrush(fill)) g.FillPath(b, path);
                GraphicsState state = g.Save();
                g.SetClip(path, CombineMode.Intersect);
                using (LinearGradientBrush sheen = new LinearGradientBrush(rect, Color.FromArgb(10, Cream), Color.FromArgb(0, Cream), 90f))
                    g.FillRectangle(sheen, rect.X, rect.Y, rect.Width, rect.Height / 2);
                g.Restore(state);
            }
            using (GraphicsPath edge = Round(Edge(rect), S(radius)))
            using (LinearGradientBrush b = new LinearGradientBrush(rect, Color.FromArgb(edgeTop, Silver), Color.FromArgb(edgeBottom, SteelDeep), 60f))
            using (Pen pen = new Pen(b, Math.Max(1f, scale)))
                g.DrawPath(pen, edge);
        }

        // The still layers, drawn once: wallpaper, shading, vignette and the glass frames.
        void EnsureBackground()
        {
            Size size = ClientSize;
            if (background != null && background.Size == size) return;
            if (background != null) background.Dispose();
            background = new Bitmap(Math.Max(1, size.Width), Math.Max(1, size.Height), PixelFormat.Format32bppPArgb);
            using (Graphics g = Graphics.FromImage(background))
            {
                g.Clear(Charcoal);
                g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                g.PixelOffsetMode = PixelOffsetMode.Half;
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.CompositingQuality = CompositingQuality.HighQuality;
                Rectangle all = new Rectangle(Point.Empty, size);
                float k = Math.Max(size.Width / (float)wallpaper.Width, size.Height / (float)wallpaper.Height);
                float ww = wallpaper.Width * k, wh = wallpaper.Height * k;
                using (ImageAttributes attributes = new ImageAttributes())
                {
                    attributes.SetWrapMode(WrapMode.TileFlipXY);
                    g.DrawImage(wallpaper, Rectangle.Round(new RectangleF((size.Width - ww) / 2, (size.Height - wh) / 2, ww, wh)),
                        0, 0, wallpaper.Width, wallpaper.Height, GraphicsUnit.Pixel, attributes);
                }
                using (LinearGradientBrush shade = new LinearGradientBrush(all, Color.FromArgb(40, 4, 8, 16), Color.FromArgb(140, 4, 8, 16), 90f))
                    g.FillRectangle(shade, all);
                using (GraphicsPath ellipse = new GraphicsPath())
                {
                    ellipse.AddEllipse(RectangleF.Inflate(all, size.Width * 0.21f, size.Height * 0.21f));
                    using (PathGradientBrush vignette = new PathGradientBrush(ellipse))
                    {
                        vignette.CenterColor = Color.FromArgb(0, 0, 0, 0);
                        vignette.SurroundColors = new Color[] { Color.FromArgb(170, 0, 0, 0) };
                        vignette.FocusScales = new PointF(0.55f, 0.45f);
                        g.FillRectangle(vignette, all);
                    }
                }
                FillGlass(g, panelRect, 14, Color.FromArgb(234, 20, 19, 22), 150, 40);
                using (GraphicsPath frame = Round(Edge(logFrame), S(8)))
                {
                    using (SolidBrush b = new SolidBrush(LogBack)) g.FillPath(b, frame);
                    using (Pen pen = new Pen(Color.FromArgb(60, Silver), Math.Max(1f, scale))) g.DrawPath(pen, frame);
                }
                FillGlass(g, footerRect, 23, Color.FromArgb(234, 18, 17, 20), 110, 35);
                FillGlass(g, pillRect, 15, Color.FromArgb(225, 18, 17, 20), 130, 60);
                using (Pen pen = new Pen(Color.FromArgb(55, Silver), Math.Max(1f, scale)))
                {
                    g.DrawLine(pen, minRect.X, pillRect.Y + S(8), minRect.X, pillRect.Bottom - S(8));
                    g.DrawLine(pen, closeRect.X, pillRect.Y + S(8), closeRect.X, pillRect.Bottom - S(8));
                }
            }
        }

        protected override void OnPaintBackground(PaintEventArgs e) { }

        protected override void OnPaint(PaintEventArgs e)
        {
            if (wallpaper == null || logo == null || scanlines == null) return;
            Graphics g = e.Graphics;
            Rectangle clip = e.ClipRectangle;
            EnsureBackground();
            g.CompositingMode = CompositingMode.SourceCopy;
            g.DrawImage(background, clip, clip, GraphicsUnit.Pixel);
            g.CompositingMode = CompositingMode.SourceOver;
            g.PixelOffsetMode = PixelOffsetMode.Half;
            g.InterpolationMode = InterpolationMode.NearestNeighbor;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.TextRenderingHint = TextRenderingHint.ClearTypeGridFit;

            if (!beamRect.IsEmpty && clip.IntersectsWith(beamRect)) DrawBeam(g);
            if (clip.IntersectsWith(GlitchArea)) DrawLogo(g);
            if (clip.IntersectsWith(pillRect)) DrawPill(g);
            if (clip.IntersectsWith(headingRect)) DrawText(g, heading, headingFont, Cream, headingRect, StringAlignment.Near);
            if (clip.IntersectsWith(statusRect)) DrawText(g, status, statusFont, statusColor, statusRect, StringAlignment.Near);
            if (showLicence) DrawLicence(g);
            else if (progressVisible && clip.IntersectsWith(Rectangle.Inflate(progressRect, S(4), S(4)))) DrawProgress(g);
            if (clip.IntersectsWith(footerRect)) DrawFooter(g);

            g.FillRectangle(scanlines, clip);
            if (glitching)
            {
                for (int i = 0; i < noise.Length; i++)
                    using (SolidBrush b = new SolidBrush(i % 2 == 0 ? Color.FromArgb(80, Ruby) : Color.FromArgb(60, Cream)))
                        g.FillRectangle(b, noise[i]);
            }
        }

        void DrawText(Graphics g, string text, Font font, Color color, Rectangle rect, StringAlignment alignment)
        {
            if (string.IsNullOrEmpty(text)) return;
            using (StringFormat format = new StringFormat(StringFormatFlags.NoWrap))
            using (SolidBrush brush = new SolidBrush(color))
            {
                format.Trimming = StringTrimming.EllipsisCharacter;
                format.LineAlignment = StringAlignment.Center;
                format.Alignment = alignment;
                g.DrawString(text, font, brush, rect, format);
            }
        }

        void DrawBeam(Graphics g)
        {
            Rectangle area = new Rectangle(beamRect.X, beamRect.Y - 1, beamRect.Width, beamRect.Height + 2);
            using (LinearGradientBrush b = new LinearGradientBrush(area, Color.Transparent, Color.Transparent, 90f))
            {
                ColorBlend blend = new ColorBlend(3);
                blend.Colors = new Color[] { Color.FromArgb(0, Ruby), Color.FromArgb(14, Ruby), Color.FromArgb(0, Ruby) };
                blend.Positions = new float[] { 0f, 0.5f, 1f };
                b.InterpolationColors = blend;
                g.FillRectangle(b, beamRect);
            }
            using (Pen pen = new Pen(Color.FromArgb(18, Ruby), 1f))
            {
                int y = beamRect.Y + beamRect.Height / 2;
                g.DrawLine(pen, 0, y, beamRect.Right, y);
            }
        }

        static Rectangle Shift(Rectangle r, int dx)
        {
            return new Rectangle(r.X + dx, r.Y, r.Width, r.Height);
        }

        void DrawLogo(Graphics g)
        {
            if (!glitching)
            {
                g.DrawImage(logo, logoRect);
                return;
            }
            g.DrawImage(logoRed, Shift(logoRect, -glitchShift));
            g.DrawImage(logoRuby, Shift(logoRect, glitchShift));
            GraphicsState state = g.Save();
            foreach (Rectangle slice in slices) g.ExcludeClip(slice);
            g.DrawImage(logo, logoRect);
            g.Restore(state);
            for (int i = 0; i < slices.Length; i++)
            {
                state = g.Save();
                g.IntersectClip(Shift(slices[i], sliceShift[i]));
                g.DrawImage(logo, Shift(logoRect, sliceShift[i]));
                g.Restore(state);
            }
        }

        void DrawPill(Graphics g)
        {
            if (hover == Hit.Minimise || hover == Hit.Close)
            {
                bool close = hover == Hit.Close;
                Color fill = close
                    ? Color.FromArgb(pressed == Hit.Close ? 170 : 120, 0xB8, 0x3A, 0x30)
                    : Color.FromArgb(pressed == Hit.Minimise ? 70 : 42, Silver);
                using (GraphicsPath path = Round(Edge(pillRect), pillRect.Height / 2f))
                using (SolidBrush b = new SolidBrush(fill))
                {
                    GraphicsState state = g.Save();
                    g.SetClip(path, CombineMode.Intersect);
                    g.FillRectangle(b, close ? closeRect : minRect);
                    g.Restore(state);
                }
            }
            DrawText(g, BuildInfo.Version, smallFont, Silver, versionRect, StringAlignment.Center);
            float width = Math.Max(1f, 1.3f * scale);
            float cx = minRect.X + minRect.Width / 2f, cy = minRect.Y + minRect.Height / 2f, half = 5f * scale;
            using (Pen pen = new Pen(Color.FromArgb(hover == Hit.Minimise ? 255 : 205, Cream), width))
                g.DrawLine(pen, cx - half, cy, cx + half, cy);
            cx = closeRect.X + closeRect.Width / 2f;
            half = 4.5f * scale;
            using (Pen pen = new Pen(Color.FromArgb(hover == Hit.Close ? 255 : 205, Cream), width))
            {
                g.DrawLine(pen, cx - half, cy - half, cx + half, cy + half);
                g.DrawLine(pen, cx - half, cy + half, cx + half, cy - half);
            }
        }

        void DrawLicence(Graphics g)
        {
            bool hot = hover == Hit.Accept;
            using (GraphicsPath path = Round(Edge(boxRect), 3f * scale))
            {
                if (accepted)
                {
                    using (LinearGradientBrush b = new LinearGradientBrush(boxRect, Silver, SteelDeep, 90f)) g.FillPath(b, path);
                }
                else
                {
                    using (SolidBrush b = new SolidBrush(Color.FromArgb(hot ? 45 : 22, Silver))) g.FillPath(b, path);
                }
                using (Pen pen = new Pen(Color.FromArgb(hot || accepted ? 235 : 170, Silver), Math.Max(1f, scale))) g.DrawPath(pen, path);
            }
            if (accepted)
            {
                float x = boxRect.X, y = boxRect.Y;
                using (Pen pen = new Pen(Charcoal, Math.Max(1.5f, 1.8f * scale)))
                {
                    pen.StartCap = LineCap.Round;
                    pen.EndCap = LineCap.Round;
                    pen.LineJoin = LineJoin.Round;
                    g.DrawLines(pen, new PointF[] {
                        new PointF(x + 3.6f * scale, y + 8.3f * scale),
                        new PointF(x + 6.7f * scale, y + 11.3f * scale),
                        new PointF(x + 12.4f * scale, y + 4.9f * scale) });
                }
            }
            DrawText(g, AcceptText, statusFont, hot ? Cream : CreamSoft, acceptTextRect, StringAlignment.Near);
            DrawText(g, LicenceText, hover == Hit.Licence ? linkFont : statusFont, Silver, licenceRect, StringAlignment.Near);
        }

        void DrawProgress(Graphics g)
        {
            using (GraphicsPath track = Round(progressRect, progressRect.Height / 2f))
            using (SolidBrush b = new SolidBrush(Color.FromArgb(160, 36, 34, 38)))
                g.FillPath(b, track);
            int width = (int)Math.Round(progressRect.Width * Math.Max(0f, Math.Min(100f, progressShown)) / 100f);
            if (width < progressRect.Height) return;
            Rectangle fill = new Rectangle(progressRect.X, progressRect.Y, width, progressRect.Height);
            bool failed = phase == Phase.Failed;
            using (GraphicsPath path = Round(fill, fill.Height / 2f))
            {
                Rectangle gradient = new Rectangle(fill.X - 1, fill.Y, fill.Width + 2, fill.Height);
                using (LinearGradientBrush b = new LinearGradientBrush(gradient, failed ? Mix(ErrorRed, SteelDark, 0.4f) : SteelDeep, failed ? ErrorRed : Silver, 0f))
                    g.FillPath(b, path);
                if (motion && phase == Phase.Running)
                {
                    int band = S(90);
                    float t = (clock.ElapsedMilliseconds % 1600) / 1600f;
                    Rectangle shine = new Rectangle(fill.X - band + (int)((fill.Width + band) * t), fill.Y, band, fill.Height);
                    GraphicsState state = g.Save();
                    g.SetClip(path, CombineMode.Intersect);
                    using (LinearGradientBrush b = new LinearGradientBrush(new Rectangle(shine.X - 1, shine.Y, shine.Width + 2, shine.Height), Color.Transparent, Color.Transparent, 0f))
                    {
                        ColorBlend blend = new ColorBlend(3);
                        blend.Colors = new Color[] { Color.FromArgb(0, Cream), Color.FromArgb(150, Cream), Color.FromArgb(0, Cream) };
                        blend.Positions = new float[] { 0f, 0.5f, 1f };
                        b.InterpolationColors = blend;
                        g.FillRectangle(b, shine);
                    }
                    g.Restore(state);
                }
            }
        }

        void DrawFooter(Graphics g)
        {
            float pulse = 1f;
            if (motion && (phase == Phase.Probing || phase == Phase.Running))
                pulse = (float)(0.5 + 0.5 * Math.Sin(clock.ElapsedMilliseconds / 260.0));
            Rectangle halo = dotRect;
            halo.Inflate(S(4), S(4));
            using (SolidBrush b = new SolidBrush(Color.FromArgb((int)(30 + 40 * pulse), dotColor))) g.FillEllipse(b, halo);
            using (SolidBrush b = new SolidBrush(dotColor)) g.FillEllipse(b, dotRect);
            string text = footer;
            if (phase == Phase.Running && progressVisible) text += " \u00B7 " + progressTarget + "%";
            DrawText(g, text, smallFont, CreamSoft, footerTextRect, StringAlignment.Near);
            if (secondaryVisible) DrawButton(g, secondaryRect, secondaryText, false, secondaryEnabled, Hit.Secondary);
            if (primaryVisible) DrawButton(g, primaryRect, primaryText, true, primaryEnabled, Hit.Primary);
        }

        void DrawButton(Graphics g, Rectangle rect, string text, bool primary, bool enabled, Hit id)
        {
            bool hot = enabled && hover == id;
            bool down = hot && pressed == id;
            using (GraphicsPath path = Round(Edge(rect), rect.Height / 2f))
            {
                if (primary)
                {
                    Color top = Mix(Ruby, Charcoal, .28f), bottom = Mix(Ruby, Charcoal, .68f);
                    if (hot) { top = Mix(Ruby, Cream, .12f); bottom = Mix(Ruby, Charcoal, .45f); }
                    if (down) { top = Mix(Ruby, Charcoal, .5f); bottom = Charcoal; }
                    int alpha = enabled ? 255 : 70;
                    using (LinearGradientBrush b = new LinearGradientBrush(rect, Color.FromArgb(alpha, top), Color.FromArgb(alpha, bottom), 90f))
                        g.FillPath(b, path);
                    if (enabled)
                    {
                        using (Pen pen = new Pen(Color.FromArgb(hot ? 120 : 70, Cream), Math.Max(1f, scale))) g.DrawPath(pen, path);
                    }
                    DrawText(g, text, buttonFont, enabled ? Cream : Color.FromArgb(150, CreamSoft), rect, StringAlignment.Center);
                }
                else
                {
                    if (hot)
                    {
                        using (SolidBrush b = new SolidBrush(Color.FromArgb(down ? 60 : 34, Silver))) g.FillPath(b, path);
                    }
                    using (Pen pen = new Pen(Color.FromArgb(enabled ? (hot ? 235 : 165) : 60, Silver), Math.Max(1f, scale))) g.DrawPath(pen, path);
                    DrawText(g, text, buttonFont, enabled ? CreamSoft : Color.FromArgb(90, CreamSoft), rect, StringAlignment.Center);
                }
            }
        }
    }
}

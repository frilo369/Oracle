using System;
using System.Drawing;
using System.IO;
using System.Reflection;
using System.Windows.Forms;

static class InstallerUiCheck
{
    const BindingFlags Fields = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
    static Assembly assembly;
    static string output;
    static int RunForm(string root, string mode, bool expectFailure)
    {
        Type optionType = assembly.GetType("Oracle.Installer.Options", true);
        object options = Activator.CreateInstance(optionType, true);
        optionType.GetField("Mode").SetValue(options, Enum.Parse(assembly.GetType("Oracle.Installer.SetupMode"), mode));
        optionType.GetField("Fixture").SetValue(options, true);
        optionType.GetField("Root").SetValue(options, root);
        if (mode == "Uninstall") optionType.GetField("Bin").SetValue(options, Path.Combine(root, "bin"));
        object session = Activator.CreateInstance(assembly.GetType("Oracle.Installer.Session"), new object[] {options});
        Form form = (Form)Activator.CreateInstance(assembly.GetType("Oracle.Installer.SetupForm"), new object[] {options, session});
        Type type = form.GetType();
        Exception failure = null;
        bool started = false;
        string last = "";
        DateTime deadline = DateTime.UtcNow.AddSeconds(30);
        using (System.Windows.Forms.Timer timer = new System.Windows.Forms.Timer())
        {
            timer.Interval = 150;
            timer.Tick += delegate {
                try
                {
                    if (DateTime.UtcNow > deadline) throw new Exception("UI check timed out in " + last);
                    string phase = type.GetField("phase", Fields).GetValue(form).ToString();
                    if (phase != last)
                    {
                        last = phase;
                        using (Bitmap image = new Bitmap(form.Width, form.Height))
                        {
                            form.DrawToBitmap(image, new Rectangle(0, 0, image.Width, image.Height));
                            image.Save(Path.Combine(output, mode + (expectFailure ? "-locked-" : "-") + phase + ".png"));
                        }
                        Console.WriteLine(mode + ": " + phase);
                    }
                    if (phase == "Ready" && !started)
                    { started = true; type.GetMethod("Primary", Fields).Invoke(form, null); }
                    else if (phase == "Failed" || phase == "Done")
                    {
                        if ((phase == "Failed") != expectFailure)
                            throw new Exception("Unexpected setup result: " + type.GetField("lastError", Fields).GetValue(form));
                        timer.Stop(); form.Close();
                    }
                }
                catch (Exception ex)
                { failure = ex; timer.Stop(); type.GetField("blockClose", Fields).SetValue(form, false); form.Close(); }
            };
            timer.Start(); Application.Run(form);
        }
        form.Dispose(); ((IDisposable)session).Dispose();
        if (failure != null) throw failure;
        return 0;
    }
    [STAThread]
    static int Main(string[] args)
    {
        try
        {
            Application.EnableVisualStyles(); Application.SetCompatibleTextRenderingDefault(false);
            assembly = Assembly.Load(File.ReadAllBytes(args[0])); output = args[1];
            Directory.CreateDirectory(output);
            string root = Path.Combine(output, "fixture");
            RunForm(root, "Install", false);
            byte[] before = File.ReadAllBytes(Path.Combine(root, @"bin\oracle.exe"));
            using (FileStream locked = new FileStream(Path.Combine(root, @"bin\oracle.exe"), FileMode.Open, FileAccess.Read, FileShare.Read))
                RunForm(root, "Install", true);
            if (Convert.ToBase64String(before) != Convert.ToBase64String(File.ReadAllBytes(Path.Combine(root, @"bin\oracle.exe"))))
                throw new Exception("Failed reinstall changed the main executable.");
            RunForm(root, "Uninstall", false);
            if (Directory.Exists(Path.Combine(root, "bin"))) throw new Exception("UI uninstall left application files.");
            Console.WriteLine("Interactive install, locked-file failure and uninstall passed. Screenshots: " + output);
            return 0;
        }
        catch (Exception ex) { Console.Error.WriteLine(ex); return 1; }
    }
}

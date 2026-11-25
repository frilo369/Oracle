using System;
using System.Drawing;
using System.IO;
using System.Reflection;
using System.Windows.Forms;
using Microsoft.Win32;

static class InstallerUiCheck
{
    const BindingFlags Fields = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
    static Assembly assembly;
    static string output;
    static void RegistrationRollback()
    {
        string keyPath=@"Software\Oracle-Audit-"+Guid.NewGuid().ToString("N");
        string link=Path.Combine(output,"test-shortcut.lnk");
        try {
            using(RegistryKey key=Registry.CurrentUser.CreateSubKey(keyPath+@"\Uninstall"))key.SetValue("DisplayVersion","old");
            using(RegistryKey key=Registry.CurrentUser.CreateSubKey(keyPath+@"\Run")){key.SetValue("Oracle",@"%TEMP%\original.exe",RegistryValueKind.ExpandString);key.SetValue("Other","untouched");}
            File.WriteAllText(link,"original link");
            Type type=assembly.GetType("Oracle.Installer.SetupEngine+RegistrationState",true);
            object snapshot=Activator.CreateInstance(type,Fields,null,new object[]{keyPath+@"\Uninstall",keyPath+@"\Run",link},null);
            using(RegistryKey key=Registry.CurrentUser.CreateSubKey(keyPath+@"\Uninstall")){key.SetValue("DisplayVersion","new");key.SetValue("NewValue",1);}
            using(RegistryKey key=Registry.CurrentUser.CreateSubKey(keyPath+@"\Run"))key.SetValue("Oracle","changed");
            File.WriteAllText(link,"changed link");
            type.GetMethod("Restore",Fields).Invoke(snapshot,null);
            using(RegistryKey key=Registry.CurrentUser.OpenSubKey(keyPath+@"\Uninstall"))
                if((string)key.GetValue("DisplayVersion")!="old"||key.GetValue("NewValue")!=null)throw new Exception("Uninstall registration rollback failed.");
            using(RegistryKey key=Registry.CurrentUser.OpenSubKey(keyPath+@"\Run"))
                if(key.GetValueKind("Oracle")!=RegistryValueKind.ExpandString||(string)key.GetValue("Oracle",null,RegistryValueOptions.DoNotExpandEnvironmentNames)!=@"%TEMP%\original.exe"||(string)key.GetValue("Other")!="untouched")throw new Exception("Startup registration rollback failed.");
            if(File.ReadAllText(link)!="original link")throw new Exception("Shortcut rollback failed.");
            Console.WriteLine("Registration rollback preserved values, types and shortcut.");
        } finally { Registry.CurrentUser.DeleteSubKeyTree(keyPath,false); }
    }
    static int RunForm(string root, string mode, bool expectFailure, string scenario = "")
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
                            image.Save(Path.Combine(output, mode + (scenario.Length>0?"-"+scenario:"") + (expectFailure ? "-locked-" : "-") + phase + ".png"));
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
            RegistrationRollback();
            string root = Path.Combine(output, "fixture");
            RunForm(root, "Install", false);
            byte[] before = File.ReadAllBytes(Path.Combine(root, @"bin\oracle.exe"));
            using (FileStream locked = new FileStream(Path.Combine(root, @"bin\oracle.exe"), FileMode.Open, FileAccess.Read, FileShare.Read))
                RunForm(root, "Install", true);
            if (Convert.ToBase64String(before) != Convert.ToBase64String(File.ReadAllBytes(Path.Combine(root, @"bin\oracle.exe"))))
                throw new Exception("Failed reinstall changed the main executable.");
            // Force failure after replacement and preservation have both started.
            string marker=Path.Combine(root,@"bin\.verify-payload");
            string extra=Path.Combine(root,@"bin\user-extra.txt");File.WriteAllText(extra,"keep this file");
            File.Delete(marker);Directory.CreateDirectory(marker);
            RunForm(root,"Install",true,"rollback");
            if(!Directory.Exists(marker)||File.ReadAllText(extra)!="keep this file"||Convert.ToBase64String(before)!=Convert.ToBase64String(File.ReadAllBytes(Path.Combine(root,@"bin\oracle.exe"))))
                throw new Exception("Late install failure did not restore the old folder and its extra files.");
            Directory.Delete(marker);File.WriteAllText(marker,"Oracle fixture");
            File.Delete(extra);
            using (FileStream locked = new FileStream(Path.Combine(root, @"bin\Uninstall.exe"), FileMode.Open, FileAccess.Read, FileShare.Read))
                RunForm(root, "Uninstall", true);
            if (Convert.ToBase64String(before) != Convert.ToBase64String(File.ReadAllBytes(Path.Combine(root, @"bin\oracle.exe"))))
                throw new Exception("Failed uninstall did not restore the main executable.");
            RunForm(root, "Uninstall", false);
            if (Directory.Exists(Path.Combine(root, "bin"))) throw new Exception("UI uninstall left application files.");
            Console.WriteLine("Interactive install, locked-file failure and uninstall passed. Screenshots: " + output);
            return 0;
        }
        catch (Exception ex) { Console.Error.WriteLine(ex); return 1; }
    }
}

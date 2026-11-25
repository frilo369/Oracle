// Oracle's per-user installer. C# 5 / .NET Framework 4.5, no extracted scripts.
using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Collections.Generic;
using System.Security.AccessControl;
using System.Security.Cryptography;
using System.Security.Principal;
using Microsoft.Win32;

namespace Oracle.Installer
{
    sealed class SetupEngine
    {
        const string UninstallKey = @"Software\Microsoft\Windows\CurrentVersion\Uninstall\Oracle";
        const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
        const string WebviewKey = @"Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
        readonly Options options;
        readonly string work;
        readonly Action<string> output;
        string Bin { get { return options.Bin ?? Path.Combine(options.Root,"bin"); } }
        string Exe { get { return Path.Combine(Bin,"oracle.exe"); } }
        internal SetupEngine(Options options,string work,Action<string> output) { this.options=options;this.work=work;this.output=output; }
        void Status(string text) { output("##status "+text); }
        void Progress(int value) { output("##progress "+value.ToString(CultureInfo.InvariantCulture)); }
        internal static void CheckPath(string path)
        {
            for (string current=Path.GetFullPath(path);current!=null;current=Path.GetDirectoryName(current))
            {
                try { if ((File.GetAttributes(current)&FileAttributes.ReparsePoint)!=0) throw new IOException("Linked setup paths are not supported."); }
                catch (FileNotFoundException) {} catch (DirectoryNotFoundException) {}
            }
        }
        static void CheckTree(string path)
        {
            CheckPath(path);
            foreach (string child in Directory.GetFileSystemEntries(path)) { CheckPath(child);if(Directory.Exists(child)) CheckTree(child); }
        }
        internal static void RemoveWork(string path) { CheckPath(path);if(Directory.Exists(path)){CheckTree(path);Directory.Delete(path,true);} }
        internal static string PrivateDirectory(string parent,string prefix)
        {
            CheckPath(parent);Directory.CreateDirectory(parent);
            string path=Path.Combine(parent,prefix+Guid.NewGuid().ToString("N"));
            DirectorySecurity security=new DirectorySecurity();security.SetAccessRuleProtection(true,false);
            SecurityIdentifier user=WindowsIdentity.GetCurrent().User;security.SetOwner(user);
            foreach(SecurityIdentifier sid in new SecurityIdentifier[]{user,new SecurityIdentifier(WellKnownSidType.LocalSystemSid,null),new SecurityIdentifier(WellKnownSidType.BuiltinAdministratorsSid,null)})
                security.AddAccessRule(new FileSystemAccessRule(sid,FileSystemRights.FullControl,InheritanceFlags.ContainerInherit|InheritanceFlags.ObjectInherit,PropagationFlags.None,AccessControlType.Allow));
            Directory.CreateDirectory(path,security);return path;
        }
        static string Hash(Stream stream) { using(SHA256 hash=SHA256.Create())return BitConverter.ToString(hash.ComputeHash(stream)).Replace("-","").ToLowerInvariant(); }
        static bool WebviewInstalled()
        {
            foreach(RegistryHive hive in new RegistryHive[]{RegistryHive.CurrentUser,RegistryHive.LocalMachine})
                foreach(RegistryView view in new RegistryView[]{RegistryView.Registry32,RegistryView.Registry64})
                    using(RegistryKey root=RegistryKey.OpenBaseKey(hive,view))using(RegistryKey key=root.OpenSubKey(WebviewKey))
                        if(key!=null){Version version;if(Version.TryParse(Convert.ToString(key.GetValue("pv")),out version)&&version.Major>0)return true;}
            return false;
        }
        Process[] OwnedProcesses()
        {
            System.Collections.Generic.List<Process> result=new System.Collections.Generic.List<Process>();
            foreach(Process process in Process.GetProcessesByName("oracle"))
            {
                try {if(string.Equals(Path.GetFullPath(process.MainModule.FileName),Path.GetFullPath(Exe),StringComparison.OrdinalIgnoreCase)){result.Add(process);continue;}}
                catch(InvalidOperationException){}catch(System.ComponentModel.Win32Exception){}
                process.Dispose();
            }
            return result.ToArray();
        }
        void CloseOracle()
        {
            Process[] running=OwnedProcesses();
            try {
                if(running.Length==0)return;
                Status("Closing Oracle...");
                ProcessStartInfo info=new ProcessStartInfo(Exe,"--quit");info.UseShellExecute=false;info.CreateNoWindow=true;
                using(Process signal=Process.Start(info)){if(!signal.WaitForExit(10000))throw new IOException("Oracle did not respond. Quit it from the tray, then retry.");}
                foreach(Process process in running)if(!process.WaitForExit(10000))throw new IOException("Quit Oracle from its tray icon, then retry.");
            } finally {foreach(Process process in running)process.Dispose();}
        }
        void Expand(string destination)
        {
            string[] manifest=Resources.Text("Oracle.payload.sha256").Replace("\r","").Split(new char[]{'\n'},StringSplitOptions.RemoveEmptyEntries);
            using(Stream source=Resources.Open("Oracle.payload.zip"))using(ZipArchive zip=new ZipArchive(source,ZipArchiveMode.Read))
            {
                if(zip.Entries.Count!=manifest.Length)throw new IOException("Setup payload count mismatch.");
                foreach(string row in manifest)
                {
                    string[] fields=row.Split('\t');if(fields.Length!=3)throw new IOException("Invalid payload manifest.");
                    string relative=fields[2].Replace('\\','/');
                    if(relative!="bin/oracle.exe"&&relative!="bin/Uninstall.exe"&&relative!="setup/MicrosoftEdgeWebview2Setup.exe")throw new IOException("Unexpected payload file.");
                    ZipArchiveEntry entry=zip.GetEntry(relative);long expected;
                    if(entry==null||!long.TryParse(fields[1],out expected)||entry.Length!=expected||entry.Length>150*1024*1024)throw new IOException("Payload size mismatch.");
                    string target=Path.Combine(destination,relative.Replace('/',Path.DirectorySeparatorChar));CheckPath(target);Directory.CreateDirectory(Path.GetDirectoryName(target));
                    using(Stream input=entry.Open())using(FileStream outputFile=new FileStream(target,FileMode.CreateNew,FileAccess.Write))input.CopyTo(outputFile);
                    using(FileStream bytes=File.OpenRead(target))if(Hash(bytes)!=fields[0])throw new IOException("Payload checksum mismatch.");
                }
            }
        }
        void Shortcut(bool create)
        {
            string path=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs),"Oracle.lnk");
            if(!create){if(File.Exists(path))File.Delete(path);return;}
            Type type=Type.GetTypeFromProgID("WScript.Shell");object shell=Activator.CreateInstance(type);object shortcut=null;
            try {
                shortcut=type.InvokeMember("CreateShortcut",BindingFlags.InvokeMethod,null,shell,new object[]{path});
                Type link=shortcut.GetType();link.InvokeMember("TargetPath",BindingFlags.SetProperty,null,shortcut,new object[]{Exe});
                link.InvokeMember("WorkingDirectory",BindingFlags.SetProperty,null,shortcut,new object[]{Bin});
                link.InvokeMember("Description",BindingFlags.SetProperty,null,shortcut,new object[]{"Oracle League companion"});
                link.InvokeMember("Save",BindingFlags.InvokeMethod,null,shortcut,null);
            }finally {if(shortcut!=null)System.Runtime.InteropServices.Marshal.FinalReleaseComObject(shortcut);System.Runtime.InteropServices.Marshal.FinalReleaseComObject(shell);}
        }
        void Register()
        {
            using(RegistryKey key=Registry.CurrentUser.CreateSubKey(UninstallKey))
            {
                key.SetValue("DisplayName","Oracle");key.SetValue("DisplayVersion",BuildInfo.Version);key.SetValue("Publisher","FrYiLo");
                key.SetValue("InstallLocation",Bin);key.SetValue("DisplayIcon",Exe);key.SetValue("UninstallString","\""+Path.Combine(Bin,"Uninstall.exe")+"\"");
                key.SetValue("NoModify",1,RegistryValueKind.DWord);key.SetValue("NoRepair",1,RegistryValueKind.DWord);
            }
            Shortcut(true);
        }
        // Registration is part of the file transaction: restore exact values and the shortcut
        // if a later registry/COM operation fails. Startup belongs to the app's settings.
        sealed class RegistryState
        {
            readonly string path,only;readonly bool existed;
            readonly Dictionary<string,object> values=new Dictionary<string,object>();
            readonly Dictionary<string,RegistryValueKind> kinds=new Dictionary<string,RegistryValueKind>();
            internal RegistryState(string path,string only)
            {
                this.path=path;this.only=only;
                using(RegistryKey key=Registry.CurrentUser.OpenSubKey(path))
                {
                    existed=key!=null;if(key==null)return;
                    foreach(string name in key.GetValueNames())if(only==null||name==only){values[name]=key.GetValue(name,null,RegistryValueOptions.DoNotExpandEnvironmentNames);kinds[name]=key.GetValueKind(name);}
                }
            }
            internal void Restore()
            {
                if(!existed&&only==null){Registry.CurrentUser.DeleteSubKeyTree(path,false);return;}
                using(RegistryKey key=Registry.CurrentUser.CreateSubKey(path))
                {
                    foreach(string name in key.GetValueNames())if((only==null||name==only)&&!values.ContainsKey(name))key.DeleteValue(name,false);
                    foreach(KeyValuePair<string,object> value in values)key.SetValue(value.Key,value.Value,kinds[value.Key]);
                }
            }
        }
        sealed class RegistrationState
        {
            readonly RegistryState uninstall,run;
            readonly string link;
            readonly byte[] bytes;
            internal RegistrationState():this(UninstallKey,RunKey,Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs),"Oracle.lnk")){}
            internal RegistrationState(string uninstallPath,string runPath,string linkPath)
            {uninstall=new RegistryState(uninstallPath,null);run=new RegistryState(runPath,"Oracle");link=linkPath;CheckPath(link);bytes=File.Exists(link)?File.ReadAllBytes(link):null;}
            internal void Restore()
            {
                List<string> errors=new List<string>();
                foreach(Action restore in new Action[]{uninstall.Restore,run.Restore,()=>{if(bytes!=null)File.WriteAllBytes(link,bytes);else if(File.Exists(link))File.Delete(link);}})
                    try{restore();}catch(Exception ex){errors.Add(ex.Message);}
                if(errors.Count>0)throw new IOException("Registration rollback failed: "+string.Join("; ",errors));
            }
        }
        static readonly string[] ManagedFiles={"oracle.exe","Uninstall.exe",".verify-payload"};
        static bool Managed(string path){return File.Exists(path)&&Array.Exists(ManagedFiles,name=>string.Equals(Path.GetFileName(path),name,StringComparison.OrdinalIgnoreCase));}
        static void MoveEntry(string source,string destination){CheckPath(source);CheckPath(destination);if(Directory.Exists(source))Directory.Move(source,destination);else File.Move(source,destination);}
        void Cleanup(string path)
        {
            try{RemoveWork(path);}catch(Exception ex){output("##warning Temporary files retained at "+path+": "+ex.Message);}
        }
        void Probe()
        {
            output("##probe installed="+(File.Exists(Exe)?FileVersionInfo.GetVersionInfo(Exe).FileMajorPart+"."+FileVersionInfo.GetVersionInfo(Exe).FileMinorPart:"none"));
            Process[] running=OwnedProcesses();output("##probe oracle="+(running.Length>0?"running":"stopped"));foreach(Process p in running)p.Dispose();
            output("##probe webview="+(WebviewInstalled()?"ok":"missing"));
        }
        void Install()
        {
            string stage=PrivateDirectory(options.Root,".oracle-stage-");string backup=null;
            try {
                if(File.Exists(Exe))
                {
                    FileVersionInfo installed=FileVersionInfo.GetVersionInfo(Exe);
                    if(new Version(installed.FileMajorPart,installed.FileMinorPart)>new Version(BuildInfo.Version))throw new IOException("A newer Oracle is already installed.");
                }
                Status("Checking Oracle's files...");Progress(15);Expand(stage);
                if(!options.Fixture&&!WebviewInstalled())
                {
                    Status("Installing Microsoft Edge WebView2 Runtime...");
                    ProcessStartInfo info=new ProcessStartInfo(Path.Combine(stage,@"setup\MicrosoftEdgeWebview2Setup.exe"),"/silent /install");info.UseShellExecute=false;info.CreateNoWindow=true;
                    using(Process runtime=Process.Start(info))if(!runtime.WaitForExit(240000)||runtime.ExitCode!=0)throw new IOException("WebView2 installation failed. Check your connection and retry.");
                    if(!WebviewInstalled())throw new IOException("WebView2 Runtime is still unavailable. Install it and retry.");
                }
                if(!options.Fixture)CloseOracle();
                Progress(65);Status("Installing Oracle...");
                RegistrationState registration=options.Fixture?null:new RegistrationState();
                if(Directory.Exists(Bin)){CheckTree(Bin);backup=Path.Combine(options.Root,".oracle-backup-"+Guid.NewGuid().ToString("N"));Directory.Move(Bin,backup);}
                List<string> preserved=new List<string>();bool placed=false;
                try {
                    Directory.Move(Path.Combine(stage,"bin"),Bin);placed=true;
                    if(backup!=null)foreach(string entry in Directory.GetFileSystemEntries(backup))if(!Managed(entry))
                    {string name=Path.GetFileName(entry);MoveEntry(entry,Path.Combine(Bin,name));preserved.Add(name);}
                    if(!options.Fixture)Register();else File.WriteAllText(Path.Combine(Bin,".verify-payload"),"Oracle fixture");
                }
                catch(Exception failure) {
                    List<string> errors=new List<string>();
                    if(registration!=null)try{registration.Restore();}catch(Exception ex){errors.Add(ex.Message);}
                    try {
                        if(backup!=null)foreach(string name in preserved)MoveEntry(Path.Combine(Bin,name),Path.Combine(backup,name));
                        if(placed&&Directory.Exists(Bin))RemoveWork(Bin);
                        if(backup!=null){Directory.Move(backup,Bin);backup=null;}
                    }catch(Exception ex){errors.Add("Files retained at "+backup+": "+ex.Message);}
                    if(errors.Count>0)throw new IOException(failure.Message+" Rollback needs attention: "+string.Join("; ",errors),failure);
                    throw;
                }
                if(backup!=null){Cleanup(backup);backup=null;}
                Progress(100);Status("Oracle "+BuildInfo.Version+" is installed.");
            } finally {Cleanup(stage);}
        }
        void Uninstall()
        {
            if(!options.Fixture)CloseOracle();
            Status("Removing Oracle...");Progress(25);
            string removed=PrivateDirectory(options.Root,".oracle-remove-");List<string> staged=new List<string>();
            RegistrationState registration=options.Fixture?null:new RegistrationState();
            try {
              foreach(string file in ManagedFiles){string path=Path.Combine(Bin,file);CheckPath(path);if(File.Exists(path)){File.Move(path,Path.Combine(removed,file));staged.Add(file);}}
              if(!options.Fixture) {
                using(RegistryKey run=Registry.CurrentUser.OpenSubKey(RunKey,true))if(run!=null)
                {
                    string current=Convert.ToString(run.GetValue("Oracle"));if(string.Equals(current,"\""+Exe+"\" --tray",StringComparison.OrdinalIgnoreCase))run.DeleteValue("Oracle",false);
                }
                Registry.CurrentUser.DeleteSubKeyTree(UninstallKey,false);Shortcut(false);
              }
            } catch(Exception failure) {
                List<string> errors=new List<string>();
                foreach(string file in staged)try{File.Move(Path.Combine(removed,file),Path.Combine(Bin,file));}catch(Exception ex){errors.Add(ex.Message);}
                if(registration!=null)try{registration.Restore();}catch(Exception ex){errors.Add(ex.Message);}
                if(errors.Count>0)throw new IOException(failure.Message+" Rollback needs attention at "+removed+": "+string.Join("; ",errors),failure);
                Cleanup(removed);throw;
            }
            Cleanup(removed);
            if(Directory.Exists(Bin)&&Directory.GetFileSystemEntries(Bin).Length==0)Directory.Delete(Bin);
            Progress(100);Status("Oracle is removed. Your preferences are preserved.");
        }
        internal int Run(string mode,bool accepted)
        {
            try {
                CheckPath(options.Root);CheckPath(Bin);
                using(System.Threading.Mutex gate=new System.Threading.Mutex(false,@"Local\Oracle-Setup"))
                {
                    bool owned=false;
                    try {try{owned=gate.WaitOne(0);}catch(System.Threading.AbandonedMutexException){owned=true;}
                        if(!owned)throw new IOException("Another Oracle setup is running.");
                        if(mode=="Probe")Probe();else if(mode=="Uninstall")Uninstall();else Install();return 0;
                    }finally {if(owned)gate.ReleaseMutex();}
                }
            }catch(Exception ex){output("##error "+ex.Message);return 1;}
        }
    }
}

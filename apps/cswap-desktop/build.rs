// Icon, version resource and manifest for the Windows exe. Explorer, the uninstall entry
// and the installer's version check read these; without them the exe shows a blank icon
// and no version.
fn main() {
    println!("cargo:rerun-if-changed=../../resources/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../resources/icon.ico")
        .set("ProductName", "Claude Swap")
        .set("FileDescription", "Claude Swap")
        .set("CompanyName", "Purwa Sabrang Ramadhan")
        .set("LegalCopyright", "Copyright © 2026 Purwa Sabrang Ramadhan")
        .set("OriginalFilename", "Claude Swap.exe")
        .set("ProductVersion", &version)
        .set("FileVersion", &version)
        // asInvoker: a per-user app never asks for elevation. PerMonitorV2: crisp text on
        // scaled displays. Common controls v6: the tray's context menu and the file
        // dialogs look native instead of Windows 95.
        .set_manifest(MANIFEST);
    if let Err(err) = res.compile() {
        panic!("could not embed the icon/version resource: {err}");
    }
}

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security><requestedPrivileges>
      <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
    </requestedPrivileges></security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application><supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/></application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
      processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
</assembly>
"#;

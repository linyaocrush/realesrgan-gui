//! Build script: on Windows, embed the application icon, version info and an
//! application manifest (PerMonitorV2 DPI awareness, UTF-8 active code page).
//!
//! Failures are reported as warnings instead of aborting the build, because
//! embedding resources requires the Windows SDK resource compiler (`rc.exe`).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../icon-256px.ico");

    #[cfg(windows)]
    windows::embed();
}

#[cfg(windows)]
mod windows {
    pub fn embed() {
        let mut res = winresource::WindowsResource::new();
        let icon = "../icon-256px.ico";
        if std::path::Path::new(icon).exists() {
            res.set_icon(icon);
        }
        res.set("ProductName", "Real-ESRGAN GUI");
        res.set("FileDescription", "Real-ESRGAN GUI");
        res.set("LegalCopyright", "AGPL-3.0");
        res.set_manifest(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>"#,
        );
        if let Err(e) = res.compile() {
            println!("cargo:warning=winresource failed to embed resources: {e}");
        }
    }
}
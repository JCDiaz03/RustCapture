//! Recursos Win32 del binario GUI: icono de la app (generado por
//! design/tools/genassets), VERSIONINFO y manifest con comctl32 v6
//! (estilos visuales de los controles comunes) + DPI per-monitor V2
//! declarativo (la llamada runtime de `dpi.rs` queda como fallback para
//! builds sin recursos, p. ej. tests).

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0" processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed=assets/rustcapture.ico");
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    // ID 1: es el que cargan tray/ventanas con LoadIconW/LoadImageW y el
    // que Explorer muestra como icono del exe (menor ID = icono de app).
    res.set_icon_with_id("assets/rustcapture.ico", "1");
    res.set("ProductName", "RustCapture");
    res.set("FileDescription", "RustCapture");
    res.set("LegalCopyright", "");
    res.set_manifest(MANIFEST);
    res.compile().expect("compilación de recursos Win32");
}

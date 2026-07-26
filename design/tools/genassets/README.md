# genassets — generador offline de assets del rediseño

Convierte los recursos de `design/` en los artefactos embebibles que consume el
producto. Los artefactos generados **se commitean**: compilar RustCapture no
requiere este tool ni sus dependencias.

## Qué genera

| Entrada | Salida |
|---|---|
| `design/icons/*.svg` (inventario `ICONOS` de `src/main.rs`) | `crates/platform-win/src/ui/iconos/atlas_{16,20,24,28,32}.bin` — máscaras de cobertura A8 concatenadas (offset del icono i = `i × lado²`), antialiasing horneado por resvg |
| ídem | `crates/platform-win/src/ui/iconos/atlas.rs` — enum `Icono` generado (no editar a mano) |
| `design/icons/app/app-icon-{16,24,32,48,256}.png` | `crates/gui/assets/rustcapture.ico` — 16/24/32/48 como DIB clásico + 256 como PNG embebido |

## Cómo regenerar

```
cd design/tools/genassets
cargo test   # formato ICO, offsets, máscaras
cargo run
```

Regenerar tras: cambiar un SVG, añadir un icono al inventario `ICONOS`
(añadir **al final** para no invalidar los índices ya usados) o cambiar el
icono de app.

## Aislamiento del workspace

El `Cargo.toml` de este crate lleva una tabla `[workspace]` vacía a propósito:
lo excluye del workspace del producto para que `resvg`/`tiny-skia` no entren
en el `Cargo.lock` de RustCapture. No añadirlo como member.

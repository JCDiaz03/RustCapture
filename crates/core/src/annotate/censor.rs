//! Censura de regiones (f.25): mosaico y desenfoque. Ambas LEEN el canvas
//! (base + anotaciones ya pintadas) y lo reescriben opaco, así que la
//! región queda censurada tal y como se ve en ese punto del z-order.
//!
//! El desenfoque copia la zona antes de escribir: sus vecindades se
//! solapan y hacerlo en sitio contaminaría las muestras siguientes. El
//! mosaico no lo necesita — sus celdas son disjuntas.
//!
//! Interno, como `shapes`: la API pública es `PixelateAnnotation`.

use crate::annotate::canvas::Canvas;
use crate::annotate::style::Color;
use crate::ports::Rect;

/// Parte del rect que cae dentro del canvas; `None` si no toca nada.
fn recortar(canvas: &Canvas, rect: Rect) -> Option<Rect> {
    Rect::new(0, 0, canvas.width(), canvas.height()).intersection(&rect)
}

/// Media RGB de una celda ya recortada al canvas.
fn media(canvas: &Canvas, x: i32, y: i32, ancho: i32, alto: i32) -> Color {
    let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
    for dy in 0..alto {
        for dx in 0..ancho {
            if let Some(c) = canvas.pixel(x + dx, y + dy) {
                r += u32::from(c.r);
                g += u32::from(c.g);
                b += u32::from(c.b);
                n += 1;
            }
        }
    }
    if n == 0 {
        return Color::rgb(0, 0, 0);
    }
    Color::rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
}

/// Mosaico: cada celda de `bloque`×`bloque` se aplana a su color medio.
/// La rejilla se ancla al origen del rect (como FastStone), no al frame.
pub(crate) fn mosaico(canvas: &mut Canvas, rect: Rect, bloque: u32) {
    let Some(zona) = recortar(canvas, rect) else {
        return;
    };
    let bloque = bloque.max(1) as i32;
    let (fin_x, fin_y) = (zona.x + zona.width as i32, zona.y + zona.height as i32);
    let mut y = zona.y;
    while y < fin_y {
        let alto = bloque.min(fin_y - y);
        let mut x = zona.x;
        while x < fin_x {
            let ancho = bloque.min(fin_x - x);
            let color = media(canvas, x, y, ancho, alto);
            for dy in 0..alto {
                for dx in 0..ancho {
                    canvas.blend_pixel(x + dx, y + dy, color);
                }
            }
            x += ancho;
        }
        y += alto;
    }
}

/// Desenfoque de caja separable: dos pasadas 1-D con sumas prefijas, de
/// coste independiente del radio. En los bordes se promedian menos
/// muestras en lugar de replicar píxeles: no aparece halo en el contorno.
pub(crate) fn desenfoque(canvas: &mut Canvas, rect: Rect, radio: u32) {
    let Some(zona) = recortar(canvas, rect) else {
        return;
    };
    let (w, h) = (zona.width as usize, zona.height as usize);
    let radio = (radio.max(1) as usize).min(w.max(h));
    let mut buffer = leer_rgb(canvas, zona);
    let mut temp = vec![0u8; buffer.len()];
    // Horizontal: h líneas de w muestras contiguas.
    blur_1d(&buffer, &mut temp, h, w, (3, w * 3), radio);
    // Vertical: w columnas de h muestras separadas por una fila.
    blur_1d(&temp, &mut buffer, w, h, (w * 3, 3), radio);
    escribir_rgb(canvas, zona, &buffer);
}

/// Copia la zona a un buffer RGB compacto (el frame es opaco: sin alfa).
fn leer_rgb(canvas: &Canvas, zona: Rect) -> Vec<u8> {
    let mut out = Vec::with_capacity(zona.width as usize * zona.height as usize * 3);
    for fila in 0..zona.height as i32 {
        for col in 0..zona.width as i32 {
            let c = canvas
                .pixel(zona.x + col, zona.y + fila)
                .unwrap_or(Color::rgb(0, 0, 0));
            out.extend_from_slice(&[c.r, c.g, c.b]);
        }
    }
    out
}

fn escribir_rgb(canvas: &mut Canvas, zona: Rect, rgb: &[u8]) {
    for fila in 0..zona.height as usize {
        for col in 0..zona.width as usize {
            let i = (fila * zona.width as usize + col) * 3;
            canvas.blend_pixel(
                zona.x + col as i32,
                zona.y + fila as i32,
                Color::rgb(rgb[i], rgb[i + 1], rgb[i + 2]),
            );
        }
    }
}

/// Una pasada de media móvil 1-D sobre RGB. Hay `lineas` líneas de
/// `largo` muestras; `pasos.0` avanza a la muestra siguiente de la línea
/// y `pasos.1` al inicio de la línea siguiente — así la misma función
/// sirve para filas y para columnas sin transponer el buffer.
fn blur_1d(
    src: &[u8],
    out: &mut [u8],
    lineas: usize,
    largo: usize,
    pasos: (usize, usize),
    radio: usize,
) {
    let (paso, salto) = pasos;
    // prefijo[k] = suma de las k primeras muestras de la línea, por canal.
    let mut prefijo = vec![0u32; (largo + 1) * 3];
    for l in 0..lineas {
        let inicio = l * salto;
        for k in 0..largo {
            let i = inicio + k * paso;
            for c in 0..3 {
                prefijo[(k + 1) * 3 + c] = prefijo[k * 3 + c] + u32::from(src[i + c]);
            }
        }
        for k in 0..largo {
            let desde = k.saturating_sub(radio);
            let hasta = (k + radio + 1).min(largo);
            let n = (hasta - desde) as u32;
            let i = inicio + k * paso;
            for c in 0..3 {
                let suma = prefijo[hasta * 3 + c] - prefijo[desde * 3 + c];
                out[i + c] = ((suma + n / 2) / n) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Frame;

    /// Frame 8×8 con mitad izquierda negra y mitad derecha blanca.
    fn mitades() -> Frame {
        let mut frame = Frame::filled(8, 8, [0, 0, 0, 255]);
        for y in 0..8u32 {
            for x in 4..8u32 {
                let i = (y as usize * 8 + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        frame
    }

    #[test]
    fn el_mosaico_aplana_cada_celda_a_su_media() {
        let mut frame = mitades();
        // Una celda de 8×8 cubre todo: media = gris medio.
        mosaico(&mut Canvas::new(&mut frame), Rect::new(0, 0, 8, 8), 8);
        let [r, g, b, a] = frame.pixel(0, 0).unwrap();
        assert!((126..=128).contains(&r) && r == g && g == b && a == 255);
        // Todos los píxeles quedan idénticos: es una sola celda.
        assert_eq!(frame.pixel(7, 7), frame.pixel(0, 0));
    }

    #[test]
    fn celdas_de_cuatro_conservan_el_contraste_entre_mitades() {
        let mut frame = mitades();
        mosaico(&mut Canvas::new(&mut frame), Rect::new(0, 0, 8, 8), 4);
        // Celda izquierda toda negra, celda derecha toda blanca.
        assert_eq!(frame.pixel(1, 1), Some([0, 0, 0, 255]));
        assert_eq!(frame.pixel(6, 6), Some([255, 255, 255, 255]));
    }

    #[test]
    fn el_mosaico_solo_toca_su_rect_y_la_ultima_celda_se_recorta() {
        // 10 px de ancho con bloque 4: celdas 4+4+2, la última recortada.
        let mut frame = Frame::filled(10, 4, [0, 0, 0, 255]);
        for x in 8..10u32 {
            for y in 0..4u32 {
                let i = (y as usize * 10 + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        mosaico(&mut Canvas::new(&mut frame), Rect::new(0, 0, 10, 4), 4);
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 255])); // celda 0-3
        assert_eq!(frame.pixel(9, 0), Some([255, 255, 255, 255])); // celda 8-9
    }

    #[test]
    fn un_rect_fuera_del_canvas_es_noop() {
        let original = mitades();
        let mut frame = original.clone();
        let mut canvas = Canvas::new(&mut frame);
        mosaico(&mut canvas, Rect::new(50, 50, 10, 10), 4);
        desenfoque(&mut canvas, Rect::new(-30, 0, 10, 10), 4);
        assert_eq!(frame, original);
    }

    #[test]
    fn el_desenfoque_difumina_el_borde_y_conserva_los_extremos() {
        let mut frame = mitades();
        desenfoque(&mut Canvas::new(&mut frame), Rect::new(0, 0, 8, 8), 2);
        // El borde negro/blanco pasa a ser un degradado monótono.
        let fila: Vec<u8> = (0..8).map(|x| frame.pixel(x, 4).unwrap()[0]).collect();
        for par in fila.windows(2) {
            assert!(par[1] >= par[0], "no es monótona: {fila:?}");
        }
        assert!(
            fila[3] > 0 && fila[4] < 255,
            "el borde no se difuminó: {fila:?}"
        );
        // Lejos del borde el color se mantiene (radio 2 no alcanza).
        assert_eq!(fila[0], 0);
        assert_eq!(fila[7], 255);
    }

    #[test]
    fn el_desenfoque_de_un_color_plano_no_lo_cambia() {
        let mut frame = Frame::filled(6, 6, [30, 60, 90, 255]);
        desenfoque(&mut Canvas::new(&mut frame), Rect::new(0, 0, 6, 6), 3);
        assert_eq!(frame, Frame::filled(6, 6, [30, 60, 90, 255]));
    }

    #[test]
    fn el_desenfoque_recorta_el_rect_desbordado_sin_panico() {
        let mut frame = mitades();
        // Rect que sale por la derecha y por abajo: se recorta a 6×6.
        desenfoque(&mut Canvas::new(&mut frame), Rect::new(2, 2, 20, 20), 3);
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 255])); // fuera del rect
        let dentro = frame.pixel(4, 4).unwrap();
        assert!(dentro[0] > 0 && dentro[0] < 255);
    }
}

//! Layout de botoneras en unidades lógicas (rejilla 4n del diseño):
//! lógica pura, testeable sin ventanas. Las cajas devueltas ya están en
//! píxeles físicos del DPI pedido.

use crate::dpi::Escala;

/// Lado del botón estándar de toolbar (lógico).
pub(crate) const BOTON: i32 = 28;
/// Espaciado base de la rejilla (lógico).
pub(crate) const ESPACIO: i32 = 4;
/// Ancho del asa de arrastre (lógico).
pub(crate) const ASA: i32 = 16;
/// Alto de la línea de un separador (lógico).
const SEPARADOR_ALTO: i32 = 20;
/// Margen a cada lado de la línea del separador (lógico).
const SEPARADOR_MARGEN: i32 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Item {
    /// Botón cuadrado de 28×28 lógicos.
    Boton,
    /// Asa de arrastre: 16 de ancho, alto de botón.
    Asa,
    /// Línea vertical de 1 lógico con márgenes.
    Separador,
    /// Hueco flexible: absorbe el ancho sobrante (0 si no se da ancho).
    Muelle,
}

/// Caja en píxeles físicos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Caja {
    pub x: i32,
    pub y: i32,
    pub ancho: i32,
    pub alto: i32,
}

/// Reparte los items en una fila de `alto_fisico` px. Devuelve una caja
/// por item (la del separador es su línea de 1 px lógico) y el ancho
/// total. Con `ancho_total` fijado, el primer `Muelle` absorbe el resto.
pub(crate) fn distribuir(
    items: &[Item],
    escala: Escala,
    alto_fisico: i32,
    ancho_total: Option<i32>,
) -> (Vec<Caja>, i32) {
    let espacio = escala.px(ESPACIO);
    let natural = ancho_natural(items, escala);
    let sobrante = ancho_total.map_or(0, |total| (total - natural).max(0));

    let mut cajas = Vec::with_capacity(items.len());
    let mut x = espacio;
    for item in items {
        let (ancho_item, caja) = match item {
            Item::Boton => {
                let lado = escala.px(BOTON);
                (lado, Caja { x, y: (alto_fisico - lado) / 2, ancho: lado, alto: lado })
            }
            Item::Asa => {
                let ancho = escala.px(ASA);
                let alto = escala.px(BOTON);
                (ancho, Caja { x, y: (alto_fisico - alto) / 2, ancho, alto })
            }
            Item::Separador => {
                let margen = escala.px(SEPARADOR_MARGEN);
                let linea = escala.px(1);
                let alto = escala.px(SEPARADOR_ALTO);
                (
                    margen * 2 + linea,
                    Caja {
                        x: x + margen,
                        y: (alto_fisico - alto) / 2,
                        ancho: linea,
                        alto,
                    },
                )
            }
            Item::Muelle => (sobrante, Caja { x, y: 0, ancho: sobrante, alto: alto_fisico }),
        };
        cajas.push(caja);
        x += ancho_item + espacio;
    }
    (cajas, ancho_total.unwrap_or(natural))
}

/// Ancho que ocupan los items sin muelle: márgenes + items + espaciados.
fn ancho_natural(items: &[Item], escala: Escala) -> i32 {
    let espacio = escala.px(ESPACIO);
    let mut ancho = espacio; // margen izquierdo
    for item in items {
        ancho += match item {
            Item::Boton => escala.px(BOTON),
            Item::Asa => escala.px(ASA),
            Item::Separador => escala.px(SEPARADOR_MARGEN) * 2 + escala.px(1),
            Item::Muelle => 0,
        } + espacio;
    }
    ancho
}

#[cfg(test)]
mod tests {
    use super::*;

    const E96: Escala = Escala::nueva(96);
    const E144: Escala = Escala::nueva(144);

    #[test]
    fn una_fila_de_botones_se_encadena_con_espaciado_4() {
        let (cajas, ancho) = distribuir(&[Item::Boton, Item::Boton], E96, 36, None);
        assert_eq!(cajas[0], Caja { x: 4, y: 4, ancho: 28, alto: 28 });
        assert_eq!(cajas[1].x, 4 + 28 + 4);
        assert_eq!(ancho, 4 + 28 + 4 + 28 + 4);
    }

    #[test]
    fn el_separador_es_una_linea_centrada_con_margenes() {
        let (cajas, _) = distribuir(&[Item::Boton, Item::Separador, Item::Boton], E96, 36, None);
        let sep = cajas[1];
        assert_eq!(sep.ancho, 1);
        assert_eq!(sep.alto, 20);
        assert_eq!(sep.x, 4 + 28 + 4 + 3); // tras el botón + espacio + margen
        assert_eq!(cajas[2].x, sep.x + 1 + 3 + 4);
    }

    #[test]
    fn el_asa_es_estrecha_y_alta_como_un_boton() {
        let (cajas, _) = distribuir(&[Item::Asa, Item::Boton], E96, 36, None);
        assert_eq!(cajas[0], Caja { x: 4, y: 4, ancho: 16, alto: 28 });
    }

    #[test]
    fn a_150_por_ciento_todo_escala() {
        let (cajas, ancho) = distribuir(&[Item::Boton, Item::Boton], E144, 54, None);
        assert_eq!(cajas[0], Caja { x: 6, y: 6, ancho: 42, alto: 42 });
        assert_eq!(cajas[1].x, 6 + 42 + 6);
        assert_eq!(ancho, 6 + 42 + 6 + 42 + 6);
    }

    #[test]
    fn el_muelle_absorbe_el_sobrante_cuando_hay_ancho_fijo() {
        let (cajas, ancho) =
            distribuir(&[Item::Boton, Item::Muelle, Item::Boton], E96, 36, Some(200));
        assert_eq!(ancho, 200);
        // natural = 4+28+4 +0+4 +28+4 = 72 → sobrante 128.
        assert_eq!(cajas[1].ancho, 128);
        assert_eq!(cajas[2].x, 4 + 28 + 4 + 128 + 4);
        // El último botón acaba pegado al margen derecho.
        assert_eq!(cajas[2].x + cajas[2].ancho + 4, 200);
    }

    #[test]
    fn sin_ancho_fijo_el_muelle_no_ocupa() {
        let (cajas, _) = distribuir(&[Item::Boton, Item::Muelle, Item::Boton], E96, 36, None);
        assert_eq!(cajas[1].ancho, 0);
        assert_eq!(cajas[2].x, 4 + 28 + 4 + 0 + 4);
    }
}

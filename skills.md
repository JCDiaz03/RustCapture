# Skills del agente — RustCapture

> **Mantenimiento de este documento — capa PROCESO (trabajo con IA).**
>
> - Qué es: contrato entre el humano y el agente sobre QUÉ skill usar en cada situación. NO documenta el producto ni su estado.
> - El agente debe leer este documento al inicio de cada sesión y respetar la delegación de la tabla; ante duda entre dos skills, gana la fila más específica.
> - Presente, sin fechas: sin "última actualización" ni "antes/ahora". El historial está en git.
> - Estado, no fecha: skills aún no instaladas o pendientes de crear se marcan — `(no instalada)`, `(pendiente de crear)`.
> - Una sola casa por dato: qué se construye → `ideas.md` · decisiones técnicas → `arquitectura.md` · fases y estado → `roadmap.md`. Este doc solo gobierna el proceso.
> - Si una skill se adopta o se descarta, se actualiza la tabla y §Descartadas con el porqué; no se deja rastro del cambio en prosa.

## Instalación

Familia única adoptada: `obra/superpowers` (skills autocontenidas, sin dependencia de issue tracker, diseñadas para componerse entre sí).

```
npx skills add obra/superpowers
```

En el instalador, seleccionar únicamente: `brainstorming`, `writing-plans`, `executing-plans`, `test-driven-development`, `systematic-debugging`, `verification-before-completion`. Agente destino: Claude Code.

Skill propia del proyecto `(pendiente de crear)`: convenciones `windows-rs`, patrones HRESULT, estructura de adapters de `platform-win` → se creará con `skill-creator` (anthropics/skills) cuando exista el esqueleto del workspace.

## Delegación: qué skill y cuándo

| Situación | Skill | Regla para el agente |
|---|---|---|
| Inicio de cualquier slice o módulo nuevo (un D.N o grupo de f.N del roadmap) | `writing-plans` | Antes de tocar código: producir un plan escrito del slice, acotado a ese slice, y validarlo con el humano. |
| Ejecución de un plan ya aprobado | `executing-plans` | Ejecutar paso a paso el plan del slice; no improvisar fuera de él sin volver a `writing-plans`. |
| Escritura de cualquier código del crate `core` | `test-driven-development` | Test primero contra los puertos/mocks (D2), código después. Obligatorio en `core`; en `platform-win` aplicar donde el adapter sea testeable sin hardware. |
| Un test falla, un HRESULT devuelve error, un frame sale negro, comportamiento inexplicable | `systematic-debugging` | Prohibido "probar cosas al azar": hipótesis → experimento mínimo → confirmar/descartar. Prioritaria en el interop Win32 (D8, adapters). |
| Antes de declarar terminada cualquier tarea o cerrar sesión | `verification-before-completion` | Compilar, correr tests, ejecutar el caso manual descrito en el plan. Sin verificación no hay "hecho" (ni ✅ en `roadmap.md`). |
| Diseño de una feature nueva o duda de enfoque sin resolver en `arquitectura.md` | `brainstorming` | Explorar opciones con el humano ANTES de escribir plan alguno; el resultado se consolida en `arquitectura.md`, no en el chat. |
| Código que toque Win32/COM/Media Foundation | Skill propia del proyecto `(pendiente de crear)` | Hasta que exista: usar `windows-rs` (nunca `winapi`), propagar errores con `windows::core::Result`, todo unsafe encapsulado en `platform-win`. |

## Orden típico de una sesión

1. Leer este documento + `roadmap.md` (fase actual).
2. `writing-plans` sobre el slice objetivo → aprobación humana.
3. `executing-plans` + `test-driven-development`.
4. `systematic-debugging` si algo se tuerce.
5. `verification-before-completion` → solo entonces proponer actualizar `roadmap.md`.

## Descartadas

- `to-spec` / `to-prd` (mattpocock): producen el spec/PRD desde la conversación, papel que ya cumplen `ideas.md` + `arquitectura.md`; además dependen de `/setup-matt-pocock-skills` (issue tracker + etiquetas), infraestructura que este proyecto no usa.
- `tdd` (mattpocock): solapa con `test-driven-development` de obra/superpowers; se adopta una sola familia para evitar doctrinas contradictorias.
- `to-tickets` (mattpocock): candidata futura solo si el proyecto adopta GitHub Issues como tracker; mientras tanto `roadmap.md` es la única fuente de tareas.
- Skills de diseño visual, web, marketing y bases de datos del directorio skills.sh: fuera de dominio (app nativa Windows, front no prioritario).

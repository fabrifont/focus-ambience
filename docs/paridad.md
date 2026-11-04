# Paridad escritorio / celu

Se actualiza en cada PR. Una función no está terminada hasta que está en las dos versiones o anotada como pendiente.

## Diferencias acordadas (etapa 1, ver wireframes aprobados en Notion)
- [x] Celu sin modo deep work (ni fila en Opciones ni «¡Buen trabajo!»).
- [x] Celu: la notificación reemplaza al daemon y a `focus pause/stop/status`, con un solo botón Pausar/Seguir que pausa sonidos **y** timer juntos (en escritorio son separados).
- [x] Celu sin Salir ni ventanita de salir; Apagar todo además cierra la notificación.
- [x] Celu sin atajos ocultos `0` (reiniciar sonidos) y `t` (pausar timer).
- [x] Celu: el timer se pausa tocando el reloj (en escritorio, tecla `t`).
- [x] Celu sin Sincronización en Configuración.
- [x] Celu sin aviso al terminar el timer: solo fade-out (escritorio manda aviso).
- [x] Celu: botones y gestos en vez de teclas, sin ayuda `?`.
- [x] Ninguno tiene «Configuración de sonidos» ni «Configuración del timer»: mezcla en Focus, timer en Opciones.

## Diferencias de la etapa 2 (propuesta del diseñador, **todavía sin aprobar por Fabrizio**)
- [x] Rutinas: en escritorio, desde Inicio se puede Editar, Borrar y «+ Nueva rutina»; en el celu, desde Inicio solo se empieza. Crear, editar y borrar va solo en Configuración (regla de Fabrizio).
- [x] Mover una tarea: botones ↑ ↓ y «Listo» en vez de flechas.
- [x] Cargar preset: sub-hoja dentro de Opciones (en escritorio, desde la lista de Opciones).
- [x] «Continuar» en Inicio muestra el nombre de la última rutina.
- [x] Celu sin meditaciones todavía: en el editor la fila es un hueco («Meditación · próximamente») y en la rutina en curso no hay paso de meditación. Una rutina que ya traía meditación la conserva al guardar.
- [x] Los sonidos de un preset que el celu no tiene (por ejemplo, `.opus` de escritorio) no suenan hasta la etapa 3.

## Decisiones pendientes de revisar
- [ ] Etapa 1 en el celu: solo ruidos generados y binaural, sin importar `.opus`; el resto llega en la etapa 3 con YouTube. Decisión de Jefe · WorkHub, **pendiente de que Fabrizio la revise**.

## Igual en los dos
- Motor de audio, mezcla, catálogo de ruidos y binaural, timer con fade de 30 s: es el mismo `core/` (`focus_core::Player`).
- Pausa de sonidos (`PauseAll`/`ResumeAll`) y Apagar todo.
- Formato de `presets.toml`, `routines.toml` y `ui.toml` (última rutina): los lee y escribe `core/`, sin cambios.
- Nombres de rutina repetidos: misma regla (`core::routine::name_taken`, sin distinguir mayúsculas).
- Durante una rutina el ambiente se pausa y vuelve al salir.

## Funciones
| Función | Escritorio | Android |
|---|---|---|
| Inicio (Continuar, Elegir rutina, Focus, Configuración) | ✅ | ✅ (#17, #19, falta celu real) |
| Antes de arrancar (minutos, preset) | ✅ | ✅ (#17, #19) |
| Focus: sonidos con volumen | ✅ | ✅ solo generados (#17) |
| Timer con fade-out | ✅ | ✅ (#17, via core) |
| Opciones (Timer, Volumen general, Pausar/Apagar todo, Volver) | ✅ | ✅ (#17) |
| Configuración > Sonidos ambientales (borrar) | ✅ | ✅ lista; borrar llega con la etapa 3 (los generados no se borran) |
| Sonido de fondo + notificación | ✅ (daemon) | ✅ servicio en primer plano (#17, falta celu real) |
| Presets (cargar, armar, borrar) | ✅ | ✅ (#19, falta celu real) |
| Rutinas / tareas (elegir, crear, editar, mover, borrar, en curso) | ✅ | ✅ (#19, falta celu real) |
| Meditaciones | ✅ | ⏳ pendiente de Fabrizio (duda 1 del diseñador) |
| Agregar desde YouTube / sonidos de archivo | ✅ | ⏳ etapa 3 |
| Sincronización con Drive | ✅ | ❌ no va |

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

## Decisiones pendientes de revisar
- [ ] Etapa 1 en el celu: solo ruidos generados y binaural, sin importar `.opus`; el resto llega en la etapa 3 con YouTube. Decisión de Jefe · WorkHub, **pendiente de que Fabrizio la revise**.

## Igual en los dos
- Motor de audio, mezcla, catálogo de ruidos y binaural, timer con fade de 30 s: es el mismo `core/` (`focus_core::Player`).
- Pausa de sonidos (`PauseAll`/`ResumeAll`) y Apagar todo.

## Funciones
| Función | Escritorio | Android |
|---|---|---|
| Inicio (Focus, Configuración) | ✅ | ✅ (#17, falta celu real) |
| Antes de arrancar (minutos) | ✅ | ✅ (#17) |
| Focus: sonidos con volumen | ✅ | ✅ solo generados (#17) |
| Timer con fade-out | ✅ | ✅ (#17, via core) |
| Opciones (Timer, Volumen general, Pausar/Apagar todo, Volver) | ✅ | ✅ (#17) |
| Configuración > Sonidos ambientales (borrar) | ✅ | ✅ lista; borrar llega con la etapa 3 (los generados no se borran) |
| Sonido de fondo + notificación | ✅ (daemon) | ✅ servicio en primer plano (#17, falta celu real) |
| Presets | ✅ | ⏳ etapa 2 |
| Rutinas / tareas | ✅ | ⏳ etapa 2 |
| Meditaciones | ✅ | ⏳ etapa 2 |
| Agregar desde YouTube / sonidos de archivo | ✅ | ⏳ etapa 3 |
| Sincronización con Drive | ✅ | ❌ no va |

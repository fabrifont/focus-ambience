# Paridad escritorio / celu

Se actualiza en cada PR. Una función no está terminada hasta que está en las dos versiones o anotada como pendiente.

## Diferencias acordadas (etapa 1, ver wireframes aprobados en Notion)
- [ ] Celu sin modo deep work (ni fila en Opciones ni «¡Buen trabajo!»).
- [ ] Celu: la notificación reemplaza al daemon y a `focus pause/stop/status`, con un solo botón Pausar/Seguir que pausa sonidos **y** timer juntos (en escritorio son separados).
- [ ] Celu sin Salir ni ventanita de salir; Apagar todo además cierra la notificación.
- [ ] Celu sin atajos ocultos `0` (reiniciar sonidos) y `t` (pausar timer).
- [ ] Celu: el timer se pausa tocando el reloj (en escritorio, tecla `t`).
- [ ] Celu sin Sincronización en Configuración.
- [ ] Celu sin aviso al terminar el timer: solo fade-out (escritorio manda aviso).
- [ ] Celu: botones y gestos en vez de teclas, sin ayuda `?`.
- [ ] Ninguno tiene «Configuración de sonidos» ni «Configuración del timer»: mezcla en Focus, timer en Opciones.

## Funciones
| Función | Escritorio | Android |
|---|---|---|
| Inicio (Focus, Configuración) | ✅ | ⏳ etapa 1 |
| Antes de arrancar (minutos) | ✅ | ⏳ etapa 1 |
| Focus: sonidos con volumen | ✅ | ⏳ etapa 1 |
| Timer con fade-out | ✅ | ⏳ etapa 1 |
| Opciones (Timer, Volumen general, Pausar/Apagar todo, Volver) | ✅ | ⏳ etapa 1 |
| Configuración > Sonidos ambientales (borrar) | ✅ | ⏳ etapa 1 |
| Sonido de fondo + notificación | ✅ (daemon) | ⏳ etapa 1 |
| Presets | ✅ | ⏳ etapa 2 |
| Rutinas / tareas | ✅ | ⏳ etapa 2 |
| Meditaciones | ✅ | ⏳ etapa 2 |
| Agregar desde YouTube | ✅ | ⏳ etapa 3 |
| Sincronización con Drive | ✅ | ❌ no va |

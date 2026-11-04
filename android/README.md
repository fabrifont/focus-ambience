# focus · Android

Esqueleto del cliente Android de focus-ambience (Kotlin + Jetpack Compose + Material3).
Por ahora solo muestra una pantalla placeholder.

## Requisitos

- JDK 17
- Android SDK 36 (platform + build-tools), con `ANDROID_HOME` o `local.properties` (`sdk.dir=...`) apuntando a él

## Compilar

```sh
./gradlew assembleDebug
```

El APK queda en `app/build/outputs/apk/debug/`.

## Qué falta

- Generar el Gradle wrapper: solo está `gradle-wrapper.properties` (Gradle 8.11.1); faltan `gradle-wrapper.jar` y los scripts `gradlew`/`gradlew.bat`.
- Los bindings UniFFI de `core/` (ver los `TODO(core)` en `app/build.gradle.kts` y `MainActivity`).
- Audio, UI real, iconos del launcher, tests y CI.

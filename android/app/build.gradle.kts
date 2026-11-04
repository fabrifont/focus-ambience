plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.fabrifont.focus"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.fabrifont.focus"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    // Bindings Kotlin generados por UniFFI (build-rust.sh).
    sourceSets.getByName("main").java.srcDir("build/generated/uniffi")
}

// Puente Rust: .so en jniLibs + bindings Kotlin. Con -PskipRust se usa lo ya generado.
val buildRust by tasks.registering(Exec::class) {
    workingDir = rootDir
    commandLine("bash", "build-rust.sh")
}

if (!project.hasProperty("skipRust")) {
    tasks.named("preBuild") { dependsOn(buildRust) }
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.media)
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    debugImplementation(libs.androidx.compose.ui.tooling)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.ext.junit)
}

import org.gradle.api.tasks.testing.logging.TestLogEvent

plugins {
    `java-library`
    jacoco
    id("com.gradle.common-custom-user-data-gradle-plugin") version "1.13"
}

java {
    sourceCompatibility = JavaVersion.VERSION_25
    targetCompatibility = JavaVersion.VERSION_25
    toolchain {
        languageVersion = JavaLanguageVersion.of(25)
    }
}

repositories {
    mavenCentral()
}

dependencies {
    // Add Java dependencies here as needed
    // Example for testing
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.0")
}

// Read properties with defaults for CI optimization
val skipRustBuild = project.findProperty("SKIP_RUST_BUILD")?.toString()?.toBoolean() ?: false
val rustTargetDir = project.findProperty("RUST_TARGET_DIR")?.toString() ?: "../target"
val rustBuildTarget = project.findProperty("RUST_BUILD_TARGET")?.toString() ?: "x86_64-unknown-linux-gnu"

// Task to build Rust library (skipped if SKIP_RUST_BUILD=true)
tasks.register<Exec>("buildRustLib") {
    description = "Build the Rust library with Cargo"
    enabled = !skipRustBuild
    if (enabled) {
        commandLine("cargo", "build", "--release", "--target", rustBuildTarget)
        workingDir = projectDir
    }
}

// Task to clean Rust build artifacts
tasks.register<Exec>("cleanRustLib") {
    description = "Clean Rust build artifacts"
    commandLine("cargo", "clean")
    workingDir = projectDir
}

// Copy Rust binary to resources
tasks.register<Copy>("copyRustLib") {
    description = "Copy compiled Rust library to build resources"
    dependsOn("buildRustLib")
    val sourceDir = if (skipRustBuild) {
        File("$rustTargetDir/$rustBuildTarget/release")
    } else {
        File("target/release")
    }
    from(sourceDir) {
        include("*.so", "*.dylib", "*.dll", "*.dll.a")
    }
    into("build/resources/main")
    doFirst {
        if (!sourceDir.exists()) {
            println("Warning: Rust build output directory does not exist: $sourceDir")
        }
    }
}

// Ensure Rust lib is built before Java compilation (unless skipped)
tasks.named("compileJava") {
    if (!skipRustBuild) {
        dependsOn("buildRustLib")
    }
}

// Process resources depends on copying Rust library
tasks.named("processResources") {
    dependsOn("copyRustLib")
}

// Test configuration
tasks.test {
    useJUnitPlatform()
    testLogging {
        events(TestLogEvent.PASSED, TestLogEvent.SKIPPED, TestLogEvent.FAILED)
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
        showStandardStreams = false
    }
}

// Java compilation configuration
tasks.compileJava {
    options.apply {
        encoding = "UTF-8"
        isDeprecation = true
        isWarnings = true
        compilerArgs.addAll(listOf(
            "-Xlint:all",
            "-Xlint:-processing",
            "--enable-preview"
        ))
    }
}

// JAR task configuration
tasks.jar {
    manifest {
        attributes(
            "Implementation-Title" to project.name,
            "Implementation-Version" to project.version,
            "Implementation-Vendor" to "Wouittone",
            "Specification-Title" to "orskit-jni",
            "Specification-Version" to project.version,
            "Bundle-License" to "MIT OR Apache-2.0"
        )
    }
}

// Code coverage with JaCoCo
jacoco {
    toolVersion = "0.8.12"
}

tasks.test {
    finalizedBy("jacocoTestReport")
}

tasks.jacocoTestReport {
    dependsOn("test")
}

// Wrapper task: ensures consistent Gradle version across developers/CI
import org.gradle.wrapper.Wrapper

tasks.register<Wrapper>("wrapper") {
    gradleVersion = "9.2"
    distributionType = Wrapper.DistributionType.BIN
}
